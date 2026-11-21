"""Reconstruct CUSA07027 trophies from the evidence saved by Cars 3."""
import re
import struct
import zlib

TROPHY_COUNT = 22
NON_PLATINUM = (1 << TROPHY_COUNT) - 2

# The CUSA07027 1.00 achievement stat checks, including those that store a
# qualifying event as a boolean rather than saving the event's full results.
TARGETS = {
    1: ("ACH_6_AIR_TRICKS", 1),
    2: ("ACH_CHALLENGE_ANY_TROPHY_COUNT", 12),
    3: ("ACH_TT_LAPS_COMPLETED", 100),
    4: ("ACH_DRIVING_BACKWARDS_SEC", 900),
    5: ("ACH_15_SIDE_BASHES_SINGLE_TAKEDOWN_EVENT", 1),
    6: ("ACH_CARS_RACED_COUNT", 21),
    7: ("ACH_IN_THE_ZONE", 30),
    8: ("ACH_70_AIR_TRICKS_SINGLE_HANG_TIME_EVENT", 1),
    9: ("ACH_15_TURBOS_USED_SINGLE_EVENT", 1),
    10: ("ACH_FEATS_COMPLETE", 50),
    11: ("ACH_STARS_COUNT", 30),
    12: ("FEAT_SIDEBASH_COUNT", 100),
    13: ("ACH_DRIVING_TWO_WHEELS_SEC", 900),
    14: ("FEAT_TRIGGER_TURBO_COUNT", 150),
    15: ("ACH_PERFECT_QUICK_START", 3),
    16: ("ACH_FEATS_COMPLETE", 136),
    17: ("ACH_MACKS_HATS", 10),
    18: ("ACH_COMPLETE_6_WAVES_SINGLE_TAKEDOWN", 1),
    19: ("ACH_IN_THE_ZONE_4_TIMES_SINGLE_EVENT", 1),
    20: ("ACH_CRASH_KARTS_DESTROYED", 1000),
    21: ("ACH_TROPHY_EVERY_EVENT", 105),
}


def unpack_player(raw):
    if len(raw) < 84 or raw[:4] != b"\x07\x02\0\0" or raw[64:68] != b"CMP1":
        return b""
    end, = struct.unpack_from("<I", raw, 4)
    unpacked, packed = struct.unpack_from("<II", raw, 68)
    if not (0 < unpacked <= 4 * 1024**2 and 84 < 84 + packed <= end <= len(raw)):
        return b""
    try:
        stream = zlib.decompressobj()
        data = stream.decompress(raw[84:84 + packed], unpacked + 1)
    except zlib.error:
        return b""
    if not stream.eof or stream.unused_data or len(data) != unpacked:
        return b""
    return data


def hex_field(profile, name):
    field = re.search(rb"(?m)^\t" + name + rb" =\s*\$([^$]+)\$", profile)
    try:
        return bytes.fromhex(field[1].decode("ascii")) if field else b""
    except ValueError:
        return b""


def earned_trophies(raw):
    profile = unpack_player(raw)
    if not profile.startswith(b"CGameSettings ="):
        return 0

    stats = {}
    player = re.search(rb"(?ms)^\tPLAYER =\s*\{(.*?)^\t\}", profile)
    if player:
        entries = re.findall(rb"(?m)^\t\t([A-Z0-9_]+) = (\d+(?:\.\d+)?)[ \t]*\r?$", player[1])
        stats = {key.decode("ascii"): float(value) for key, value in entries if len(value) <= 32}

    count = len({int(feat) for feat in re.findall(
        rb"(?m)^\tFeatId\d+ = ([0-9]{1,3})\r?\n\tFeatUnlocked\d+ = 1\r?$", profile)
        if 1 <= int(feat) <= 136})
    stats["ACH_FEATS_COMPLETE"] = max(stats.get("ACH_FEATS_COMPLETE", 0), count)

    results = hex_field(profile, b"m_trophyStatus")
    if len(results) == 345 and all(stars <= 3 for stars in results):
        # The title uses the best result over three difficulty levels for each
        # of the 105 events on its 21 regular tracks.
        best = [max(results[i:i + 3]) for i in range(0, 315, 3)]
        stats["ACH_TROPHY_EVERY_EVENT"] = max(stats.get("ACH_TROPHY_EVERY_EVENT", 0), sum(stars > 0 for stars in best))
        stats["ACH_STARS_COUNT"] = max(stats.get("ACH_STARS_COUNT", 0), sum(best))

    persistent = hex_field(profile, b"PERSISTENTDATA")
    flags = {}
    if len(persistent) % 64 == 0:
        for i in range(0, len(persistent), 64):
            key, user, value = struct.unpack_from("<56sII", persistent, i)
            if user == 0xffffffff:
                flags[key.split(b"\0", 1)[0]] = value
    medals = (flags.get(b"ChallengeTrophies", 0) & 0xfff).bit_count()
    cars = (flags.get(b"CarsRaced", 0) & ((1 << 23) - 1) & ~((1 << 6) | (1 << 11))).bit_count()
    stats["ACH_CHALLENGE_ANY_TROPHY_COUNT"] = max(stats.get("ACH_CHALLENGE_ANY_TROPHY_COUNT", 0), medals)
    stats["ACH_CARS_RACED_COUNT"] = max(stats.get("ACH_CARS_RACED_COUNT", 0), cars)

    unlocked = flags.get(b"ACH_BITMASKS_1", 0) & NON_PLATINUM
    for trophy, (stat, target) in TARGETS.items():
        if stats.get(stat, 0) >= target:
            unlocked |= 1 << trophy
    return unlocked
