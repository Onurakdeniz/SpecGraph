# Existing Repository Adoption — Happy Path

Run `sg adopt scan --mode observe` on an existing repo. Expected result: the scan
produces untrusted observations and findings without blocking development. Move
new governed files to `enforce-new-work`, then to strict mode after baseline facts
exist.
