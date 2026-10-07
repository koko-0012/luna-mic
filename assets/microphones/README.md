# Microphone artwork

The app detects brands from Windows endpoint names using `profiles.json`.
The current HyperX profile also recognizes QuadCast / QuadCast S / QuadCast 2 /
QuadCast 2 S / SoloCast for artwork selection. This is name matching, not a
verified USB hardware identity.

All devices currently fall back to `generic/microphone.svg`: the transparent
outline matching the supplied reference, now tinted soft purple for the Luna Mic
theme. No manufacturer photos are bundled.

To add a properly licensed transparent product image:

1. Place a PNG, WebP or SVG under its brand folder, e.g.
   `assets/microphones/hyperx/quadcast.webp`.
2. Set the corresponding model's `image` in `profiles.json` to
   `"hyperx/quadcast.webp"`. List specific model matches before broader ones.
3. Include the source URL, author, license and redistribution terms alongside
   the asset, and add its attribution to `THIRD_PARTY_NOTICES.md`.
4. Rebuild. Vite bundles the local image; the app never downloads images at runtime.

A model with no image, unknown brand, missing asset, or failed image load uses
the generic icon. A different model's photo is never used as the fallback for a
recognized model. An optional top-level `image` on a brand profile is reserved
for a licensed brand illustration when no specific model matches.
