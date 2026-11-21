//! Read-only view of a playable PS4 package. Node IDs stay valid for the mount's lifetime.
mod trophies;

use anyhow::{Context, Result, bail, ensure};
use memmap2::Mmap;
use orbis_pfs::{
    directory::{DirEntry, Directory},
    file::File,
    pfsc::PfscImage,
};
use orbis_pkg::Pkg;
use self_cell::self_cell;
use std::{
    collections::HashMap,
    ffi::{CStr, OsString, c_char},
    fs::OpenOptions,
    os::windows::{ffi::OsStringExt, fs::OpenOptionsExt},
    panic::{AssertUnwindSafe, catch_unwind},
    path::PathBuf,
    slice,
};

enum Contents<'a> {
    Directory(Vec<u32>),
    File(File<'a>),
    Metadata(Vec<u8>),
}
struct Node<'a> {
    name: String,
    contents: Contents<'a>,
    mtime: u64,
}
struct Index<'a> {
    nodes: Vec<Node<'a>>,
    paths: HashMap<String, u32>,
}
struct Backing {
    pkg: Pkg<Mmap>,
    _read_lease: std::fs::File,
}
self_cell!(
    struct Archive {
        owner: Backing,
        #[covariant]
        dependent: Index,
    }
);

impl<'a> Index<'a> {
    fn insert(
        &mut self,
        parent: u32,
        path: &str,
        name: &str,
        contents: Contents<'a>,
        mtime: u64,
    ) -> Result<u32> {
        ensure!(
            self.nodes.len() < 200_000
                && !name.is_empty()
                && name.len() < 256
                && !matches!(name, "." | "..")
                && !name.contains(['/', '\\', '\0']),
            "invalid directory entry"
        );
        let id = self.nodes.len() as u32;
        ensure!(
            self.paths.insert(path.to_ascii_lowercase(), id).is_none(),
            "duplicate path: {path}"
        );
        self.nodes.push(Node {
            name: name.to_owned(),
            contents,
            mtime,
        });
        if let Contents::Directory(children) = &mut self.nodes[parent as usize].contents {
            children.push(id);
        } else {
            bail!("invalid parent");
        }
        Ok(id)
    }
    fn walk(&mut self, dir: Directory<'a>, parent: u32, prefix: &str, depth: usize) -> Result<()> {
        ensure!(depth < 64, "invalid directory nesting");
        for (raw, entry) in dir.open()? {
            let name = std::str::from_utf8(&raw)?;
            let path = if prefix.is_empty() {
                name.to_owned()
            } else {
                format!("{prefix}/{name}")
            };
            match entry {
                DirEntry::Directory(d) => {
                    let id =
                        self.insert(parent, &path, name, Contents::Directory(vec![]), d.mtime())?;
                    self.walk(d, id, &path, depth + 1)?;
                }
                DirEntry::File(f) => {
                    let mt = f.mtime();
                    self.insert(parent, &path, name, Contents::File(f), mt)?;
                }
                _ => bail!("unknown directory entry"),
            }
        }
        Ok(())
    }
    fn build(pkg: &'a Pkg<Mmap>) -> Result<Self> {
        let title_id = pkg.header().content_id().title_id();
        ensure!(
            title_id == "CUSA07027",
            "Detected {title_id}. Expected Cars 3 CUSA07027."
        );
        let image = pkg.get_pfs_image().context("missing readable PFS")?;
        let outer = orbis_pfs::open_slice(image.data, Some(image.ekpfs))?;
        let Some(DirEntry::Directory(root)) = outer.root().open()?.remove(b"uroot") else {
            bail!("missing outer root")
        };
        let Some(DirEntry::File(inner)) = root.open()?.remove(b"pfs_image.dat") else {
            bail!("missing inner image")
        };
        let pfs = if inner.is_compressed() {
            orbis_pfs::open_image(PfscImage::open(inner.into_image())?)?
        } else {
            orbis_pfs::open_image(inner.into_image())?
        };
        let Some(DirEntry::Directory(root)) = pfs.root().open()?.remove(b"uroot") else {
            bail!("missing game root")
        };
        let mut result = Index {
            nodes: vec![Node {
                name: String::new(),
                contents: Contents::Directory(vec![]),
                mtime: root.mtime(),
            }],
            paths: HashMap::from([(String::new(), 0)]),
        };
        result.walk(root, 0, "", 0)?;
        // Public package metadata supplements the inner filesystem. The inner PFS takes precedence.
        for entry in pkg.entries() {
            let (_, entry) = entry?;
            let Some(path) = entry.to_path("sce_sys") else {
                continue;
            };
            let path = path.to_string_lossy().replace('\\', "/");
            if !(matches!(
                path.as_str(),
                "sce_sys/param.sfo"
                    | "sce_sys/icon0.png"
                    | "sce_sys/pic0.png"
                    | "sce_sys/pic1.png"
                    | "sce_sys/playgo-chunk.dat"
            ) || path.starts_with("sce_sys/trophy/"))
                || result.paths.contains_key(&path)
            {
                continue;
            }
            let data = pkg.entry_data(&entry)?;
            let mut parent = 0;
            let mut prefix = String::new();
            let mut parts = path.split('/').peekable();
            while let Some(name) = parts.next() {
                if !prefix.is_empty() {
                    prefix.push('/');
                }
                prefix.push_str(name);
                if parts.peek().is_none() {
                    result.insert(parent, &prefix, name, Contents::Metadata(data), 0)?;
                    break;
                }
                parent = if let Some(&idx) = result.paths.get(&prefix) {
                    idx
                } else {
                    result.insert(parent, &prefix, name, Contents::Directory(vec![]), 0)?
                };
            }
        }
        Ok(result)
    }
}

#[repr(C)]
pub struct Info {
    pub length: u64,
    pub mtime: u64,
    pub children: u32,
    pub directory: u32,
}
fn info(n: &Node<'_>) -> Info {
    let (length, children, directory) = match &n.contents {
        Contents::File(f) => (f.len(), 0, 0),
        Contents::Metadata(b) => (b.len() as u64, 0, 0),
        Contents::Directory(c) => (0, c.len() as u32, 1),
    };
    Info {
        length,
        mtime: n.mtime,
        children,
        directory,
    }
}

/// All pointers are supplied by the paired PistonPort reader. An archive outlives its readers.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pp_open(
    path: *const u16,
    count: usize,
    error: *mut u8,
    capacity: usize,
) -> *mut std::ffi::c_void {
    let result = catch_unwind(AssertUnwindSafe(|| -> Result<Archive> {
        ensure!(!path.is_null() && count < 32768, "invalid package path");
        let path = PathBuf::from(OsString::from_wide(unsafe {
            slice::from_raw_parts(path, count)
        }));
        // Deny concurrent writes/deletion while any mapped view is live.
        let file = OpenOptions::new().read(true).share_mode(1).open(&path)?;
        ensure!(file.metadata()?.len() >= 0x1000, "incomplete package");
        let pkg = Pkg::new(unsafe { Mmap::map(&file)? })?;
        Archive::try_new(
            Backing {
                pkg,
                _read_lease: file,
            },
            |owner| Index::build(&owner.pkg),
        )
    }))
    .unwrap_or_else(|_| Err(anyhow::anyhow!("invalid package layout")));
    match result {
        Ok(a) => Box::into_raw(Box::new(a)).cast(),
        Err(e) => {
            if !error.is_null() && capacity > 0 {
                let s = format!("{e:#}");
                let n = s.len().min(capacity - 1);
                unsafe {
                    std::ptr::copy_nonoverlapping(s.as_ptr(), error, n);
                    *error.add(n) = 0;
                }
            }
            std::ptr::null_mut()
        }
    }
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pp_close(a: *mut std::ffi::c_void) {
    if !a.is_null() {
        drop(unsafe { Box::from_raw(a.cast::<Archive>()) });
    }
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pp_find(
    a: *const std::ffi::c_void,
    path: *const c_char,
    out: *mut Info,
) -> i32 {
    let index = unsafe { &*a.cast::<Archive>() }.borrow_dependent();
    let Ok(path) = unsafe { CStr::from_ptr(path) }.to_str() else {
        return -1;
    };
    let key = path
        .split('/')
        .filter(|s| !s.is_empty() && *s != ".")
        .collect::<Vec<_>>()
        .join("/")
        .to_ascii_lowercase();
    let Some(&id) = index.paths.get(&key) else {
        return -1;
    };
    unsafe {
        *out = info(&index.nodes[id as usize]);
    }
    id as i32
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pp_child(
    a: *const std::ffi::c_void,
    id: u32,
    which: u32,
    name: *mut u8,
    capacity: usize,
) -> i32 {
    let index = unsafe { &*a.cast::<Archive>() }.borrow_dependent();
    let Some(Node {
        contents: Contents::Directory(children),
        ..
    }) = index.nodes.get(id as usize)
    else {
        return -1;
    };
    let Some(&child) = children.get(which as usize) else {
        return -1;
    };
    let n = &index.nodes[child as usize];
    if capacity <= n.name.len() {
        return -1;
    }
    unsafe {
        std::ptr::copy_nonoverlapping(n.name.as_ptr(), name, n.name.len());
        *name.add(n.name.len()) = 0;
    }
    if matches!(n.contents, Contents::Directory(_)) {
        4
    } else {
        8
    }
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pp_read(
    a: *const std::ffi::c_void,
    id: u32,
    off: u64,
    buffer: *mut u8,
    len: usize,
) -> i64 {
    if len == 0 {
        return 0;
    }
    if buffer.is_null() || len > isize::MAX as usize {
        return -1;
    }
    catch_unwind(AssertUnwindSafe(|| {
        let index = unsafe { &*a.cast::<Archive>() }.borrow_dependent();
        let Some(node) = index.nodes.get(id as usize) else {
            return -1;
        };
        let buf = unsafe { slice::from_raw_parts_mut(buffer, len) };
        match &node.contents {
            Contents::File(f) => f.read_at(off, buf).map(|n| n as i64).unwrap_or(-1),
            Contents::Metadata(b) => {
                let from = off.min(b.len() as u64) as usize;
                let n = len.min(b.len() - from);
                buf[..n].copy_from_slice(&b[from..from + n]);
                n as i64
            }
            _ => -1,
        }
    }))
    .unwrap_or(-1)
}
