# Virtual audio setup

Luna Mic uses the standard VB-CABLE by VB-Audio. The app requires both cable
endpoints, offers the original signed installer when missing, and routes processed
voice to CABLE Input. Voice apps use CABLE Output as their microphone.

`install-vb-cable.ps1` verifies the vendor publisher signature and opens the original
interactive setup only after the user presses Install. It never silently installs
or changes driver-signing/boot settings. The vendor setup requests administrator
permission and instructs users to restart Windows afterward.

VB-CABLE is proprietary donationware. Original license and files are preserved;
see `vendor/VB-CABLE-NOTICE.md` and `vendor/vb-cable/readme.txt`.
