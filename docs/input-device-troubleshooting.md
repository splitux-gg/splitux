# Input-device troubleshooting: games that crash on controller init

Some games — especially older Unity titles with a **statically-linked, ancient SDL2**
(e.g. *Overcooked! 2*, which uses InControl over Unity legacy input) — crash at startup
when a **non-gamepad input device is mis-tagged as a joystick** by udev.

## Symptom

- The game window appears for a moment, then gamescope logs `Primary child shut down!`
- The game's `Player.log` (or stdout) shows a native crash:
  - `UnityEngine.Input.GetJoystickNames()` → `SIGABRT`, often via `InControl.UnityInputDeviceManager.QueryJoystickInfo`
  - native frame in `libc` `strlen`/`memcpy` with a NULL pointer
- It launches fine with **one or zero** controllers but crashes with **two**, or crashes
  even with one controller on some hosts.

## Cause

udev's `input_id` builtin sets `ID_INPUT_JOYSTICK=1` on any device that *looks* joystick-ish.
Some keyboards/mice expose extra HID endpoints (a "System Control", "Consumer Control", a
vendor control interface, etc.) that get this tag even though they are **not** gamepads.

SDL then enumerates that phantom device as a joystick. A modern SDL handles it gracefully,
but a game's **old bundled SDL** can return a `NULL` name for it (or for a later index),
and the game does `std::string(NULL)` → `strlen(NULL)` → crash. Steam's own builds dodge
this because the Steam Linux Runtime container never exposes the phantom device that way.

> Real example: the ZSA Moonlander keyboard's **"System Control"** endpoint is tagged
> `ID_INPUT_JOYSTICK=1`. With it present, *Overcooked! 2* crashed the instant a real pad
> was also connected. Untagging it = clean boot, 2-controller couch co-op works.

## Diagnose: list non-gamepad devices tagged as joysticks

```bash
for e in /dev/input/event*; do
  if udevadm info -q property -n "$e" | grep -q 'ID_INPUT_JOYSTICK=1'; then
    name=$(udevadm info -q property -n "$e" | sed -n 's/^NAME=//p')
    [ -z "$name" ] && name=$(cat "/sys/class/input/$(basename "$e")/device/name" 2>/dev/null)
    echo "$e  $name"
  fi
done
```

Anything in that list that is **not an actual game controller** (your keyboard, mouse,
trackball, a "System/Consumer Control" endpoint, etc.) is a candidate to untag.

To confirm a specific device is the culprit, capture which one returns a NULL name:

```bash
# tiny LD_PRELOAD that logs every /dev/input open + EVIOCGNAME result — see scripts/
```

## Fix: untag the non-gamepad device (host udev rule)

Copy the template and edit the device name(s) to match yours, then install:

```bash
sudo cp docs/udev/99-splitux-not-joystick.rules.example \
        /etc/udev/rules.d/99-splitux-not-joystick.rules
sudo nano /etc/udev/rules.d/99-splitux-not-joystick.rules   # set ATTRS{name}==...
sudo udevadm control --reload-rules
sudo udevadm trigger --subsystem-match=input
```

Find the exact `name` to match with:

```bash
cat /sys/class/input/eventNN/device/name
```

This only removes the bogus *joystick* label — the device keeps working normally
(your keyboard still types, media keys still work). It fixes **every** game, not just
the one that crashed, and survives reboots.

## Notes

- This is host-side and per-user (the offending device is specific to your hardware),
  which is why it lives in `/etc/udev/rules.d/` rather than in a handler.
- If you'd rather not touch `/etc`, a future splitux option can mask non-gamepad
  joystick devices from games at launch time (see the repo issues / TODO).
