# VB-CABLE distribution provenance

The standard VB-CABLE Pack45 was downloaded from VB-Audio's official URL:
https://download.vb-audio.com/Download_CABLE/VBCABLE_Driver_Pack45.zip
on 7 October 2026. The extracted upstream files are unmodified. The original
readme/end-user license and all original icons, installers and driver files remain.
This is proprietary donationware, not covered by Luna Mic's MIT license.

VB-Audio permits bundling the standard cable with an application when its origin
and donationware model are clearly identified and users can donate/pay if useful:
https://vb-audio.com/Services/licensing.htm#
See the section “VB-CABLE Distribution with other product”. Its permission does
not cover the paid A+B/C+D cables, which are not included here.

Luna Mic prominently names VB-Audio, explains donationware and links the official
website in its setup dialog. Installation opens the original vendor setup with
administrator consent. It does not rebrand, silently install, modify or remove
VB-CABLE. Vendor setup asks users to complete installation and restart Windows.

The x64 installer SHA256 is:
734C35DFA6D98F48782A451633CEB471166EC70D60482FD89A1123D0EE3C4F41
Its Authenticode signature was valid for BUREL VINCENT Entrepreneur individuel.
The Windows 10 x64 catalog signature was valid for Microsoft Windows Hardware
Compatibility Publisher. Runtime launch rechecks the original installer signature.

The separate install-vb-cable.ps1 wrapper belongs to Luna Mic. No signing
certificate, SDK or payment to Luna Mic is needed to launch vendor setup.
Luna Mic's own unsigned driver remains development source only.
