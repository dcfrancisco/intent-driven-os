# WP-0114: MarinaOS package split

## Outcome

Produce matching marinaos-core and marinaos-cli amd64 DEBs. Core owns
marinaosd; CLI owns marinaosctl; Talvik is absent and optional.

## Evidence required

Build both packages from pinned inputs, record SHA-256 hashes, inspect package
contents, install both on clean Ubuntu 24.04, and test upgrade/removal data
retention. Current status is SOURCE_PRESENT; no Ubuntu execution is claimed.
