# CI/CD

GitHub Actions publishes release packages for this repository when a GitHub Release is published.

## Trigger

- Workflow: `.github/workflows/publish.yml`
- Event: `release.published`

## Release gate

The publish workflow runs these checks before any release asset is built:

- `cargo test -p kaulan`
- `npm run test -- --run`
- `npm run build`

If any of them fail, no release packages are uploaded.

## Release outputs

The workflow uploads these packages to the GitHub Release:

- Windows desktop bundles
- Linux desktop bundles
- Android APK files
- Android AAB files

## Android signing secrets

Android CI release builds require repository secrets:

- `ANDROID_KEY_ALIAS`
- `ANDROID_KEY_PASSWORD`
- `ANDROID_KEY_BASE64`

`ANDROID_KEY_BASE64` must contain the base64-encoded contents of your Android upload keystore.

Example:

```bash
base64 -i /path/to/upload-keystore.jks
```

## Publish flow

1. Push a version tag.
2. Create a GitHub Release for that tag.
3. Publish the release.
4. GitHub Actions runs tests.
5. If tests pass, Windows, Linux, and Android release packages are built and attached.
