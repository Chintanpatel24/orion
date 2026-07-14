# Publishing to GitHub

## Create the repository

Create an empty GitHub repository named `orion`.

## Initialize git locally

From this folder:

```sh
git init
git add .
git commit -m "initial orion ide repository"
git branch -M main
git remote add origin https://github.com/your-name/orion.git
git push -u origin main
```

## Update installer URLs

Replace `https://github.com/orion-ide/orion.git` in these files if you publish under another account:

- `scripts/install.sh`
- `scripts/install.ps1`
- `README.md`
- `docs/INSTALL.md`

## Create a release

Tag a version:

```sh
git tag v0.2.0
git push origin v0.2.0
```

The release workflow builds binaries for Windows, Linux, and macOS and uploads artifacts.
