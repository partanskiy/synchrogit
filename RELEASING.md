# Releasing synchrogit

Release automation lands in the release and AUR pipeline PRs. Until then, releases are not cut from this repository.

The intended flow is:

1. Land a `release: vX.Y.Z` commit on `main`.
2. Push the matching `vX.Y.Z` tag together with that commit.
3. Let the release workflow build flat binary tarballs.
4. Let the AUR workflow render and publish `synchrogit` and `synchrogit-bin`.

The atomic push shape is:

```sh
git push --atomic origin main vX.Y.Z
```

The release pipeline PR will make this document authoritative.
