# macOS Code Signing & Notarization

CI signs and notarizes the macOS build automatically. It needs five secrets,
provided as organization secrets shared with this repository (or as repository
secrets under **Settings → Secrets and variables → Actions**).

The macOS build job imports the certificate, derives the signing identity from
the keychain, and hands the App Store Connect API key to Tauri for notarization.
When these secrets are absent (for example on a fork), the macOS build still
runs but produces an **unsigned** app.

## 1. Developer ID Application certificate

On a Mac where the certificate is installed, in **Keychain Access**:

1. Find **Developer ID Application: \<Your Name/Team\>** under *My Certificates*.
2. Right-click → **Export** → save as `cert.p12`, set a password.
3. Base64-encode it:
   ```bash
   base64 -i cert.p12 | pbcopy
   ```

- `MACOS_CERTIFICATE` — the base64 string from above.
- `MACOS_CERTIFICATE_PASSWORD` — the password you set during export.

> The workflow derives `APPLE_SIGNING_IDENTITY` automatically from the imported
> certificate (the first `Developer ID Application` identity it finds), so there
> is no separate identity secret to manage.

## 2. App Store Connect API key (notarization)

In **App Store Connect → Users and Access → Integrations → App Store Connect API**:

1. Create a key with the **Developer** role (or higher). Download the
   `AuthKey_XXXXXXXXXX.p8` (downloadable only once).
2. Base64-encode it:
   ```bash
   base64 -i AuthKey_XXXXXXXXXX.p8 | pbcopy
   ```

- `APPLE_API_KEY` — the base64 string from above (the `.p8` contents).
- `APPLE_API_KEY_ID` — the 10-character Key ID shown next to the key.
- `APPLE_API_ISSUER` — the Issuer ID (UUID) shown above the keys list.

## Verifying a release

After a tagged release builds, download the `.dmg`, install the app, then:

```bash
spctl -a -vvv -t install /Applications/Notefix.app   # => accepted, source=Notarized Developer ID
xcrun stapler validate /Applications/Notefix.app     # => The validate action worked!
```

## Local builds

`npm run tauri build` with none of the above set produces an **unsigned** build.
To sign and notarize locally, export the equivalent Tauri environment variables
(`APPLE_CERTIFICATE`, `APPLE_CERTIFICATE_PASSWORD`, `APPLE_SIGNING_IDENTITY`,
`APPLE_API_KEY`, `APPLE_API_KEY_ID`, `APPLE_API_ISSUER`); see the
[Tauri macOS signing guide](https://v2.tauri.app/distribute/sign/macos/).
