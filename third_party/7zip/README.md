# 7-Zip Sidecar compliance record

QZip M1 uses the official **7-Zip 26.02 Windows x64** command-line runtime.
`manifest.json` pins the official MSI and source archive URLs and their
SHA-256 hashes. The MSI is used only to extract `7z.exe` and its adjacent
`7z.dll`; it is never installed and the extracted files are ignored by Git.

Run `pnpm sidecar:fetch` to download, hash-verify and extract the runtime and
to retain the matching verified source archive locally. Run `pnpm
sidecar:verify` to check both artifacts. The downloaded files are ignored by
Git; their source URLs and checksums remain in the committed manifest. QZip
does not modify 7-Zip. 7-Zip is distributed under GNU LGPL 2.1 or later, with the unRAR
restriction; see https://www.7-zip.org/faq.html for the official licence FAQ.
