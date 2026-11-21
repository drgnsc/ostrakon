# Ostrakon

Lokalny podgląd plików Markdown w stylu Orphalese. Ostrakon to odłamek naczynia, na którym w starożytności pisano. Ikona to taki odłamek z fenicką literą *mem*, przodkinią litery M.

Tylko odczyt. Tauri 2 + Rust, frontend bez frameworka i bez zależności npm w runtime.

## Skróty

| Klawisz | Działanie |
|---|---|
| `Ctrl+P` | archiwum: szukanie po wszystkich `.md` w dodanych folderach (domyślnie `C:\Projekty`) |
| `Ctrl+O` | otwórz plik |
| `Ctrl+B` | pokaż lub ukryj panel boczny |
| `Ctrl+T` | jasny lub ciemny kamień |
| `Ctrl+` `+` / `-` / `0`, `Ctrl`+kółko | rozmiar tekstu |
| `Alt+←`, boczny przycisk myszy | poprzedni dokument |
| `F5`, `Ctrl+R` | odczytaj ponownie (zmiany na dysku i tak wczytują się same) |
| `Ctrl+W` | zamknij |

Pliki można też przeciągnąć na okno albo otworzyć dwuklikiem. Każdy dokument ma własny, stały układ pęknięć wyliczony ze ścieżki.

## Model bezpieczeństwa

- Frontend nie ma dostępu do dysku, powłoki ani sieci. W `src-tauri/capabilities/main.json` są tylko uprawnienia do paska tytułu i odbioru zdarzeń.
- Rust otwiera wyłącznie pliki Markdown do 16 MB. Ścieżki UNC odrzuca przed dotknięciem dysku, bo samo ich rozwiązanie wysłałoby poświadczenia NTLM do obcego hosta.
- HTML przechodzi przez sanitizer ammonia, a CSP (`default-src 'none'`) jest drugą, niezależną warstwą.
- Zdalne obrazy są blokowane, więc nie działają piksele śledzące. Lokalne obrazy są serwowane tylko z projektu otwartego dokumentu.
- Linki `http`, `https` i `mailto` otwierają się w przeglądarce dopiero po kliknięciu. Linki do innych typów plików są blokowane.
- Okno nie może przejść na inny adres (`navigation-guard` w `src-tauri/src/lib.rs`).
- Brak telemetrii i automatycznych aktualizacji.

Aplikacja zapisuje tylko listę folderów i indeks archiwum (ścieżki plików) w `%APPDATA%\pl.orphalese.ostrakon`.

## Budowanie

```bash
pnpm install
pnpm build
```

Testy:

```bash
cd src-tauri && cargo test --lib
```

Faktury i ikona są generowane przez `pnpm assets` (`scripts/make_assets.py`). Instalator trafia do `src-tauri/target/release/bundle/nsis/`.

Jeśli ścieżka projektu jest bardzo długa, linker MSVC przekroczy limit 260 znaków (`LNK1104`). Wtedy ustaw `CARGO_TARGET_DIR` na krótki folder.

Avast (Autosandbox) potrafi bez komunikatu zatrzymać niepodpisane pliki `.exe` przy starcie: proces zostaje w stanie `Initialized` z 0 s CPU, a build lub program wisi bez końca. Pomagają wyjątki na konkretne pliki: skrypty `release\build\*\build-script-build.exe`, `ostrakon.exe` i instalator.
