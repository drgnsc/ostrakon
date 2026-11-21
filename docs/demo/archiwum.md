# Archiwum

Archiwum (`Ctrl+P`) przeszukuje pliki Markdown we wskazanych folderach. Przy pierwszym uruchomieniu lista folderów jest pusta, a okno archiwum podpowiada przycisk „Dodaj folder”.

## Dodawanie i usuwanie folderów

Przycisk „Dodaj folder” w stopce okna archiwum otwiera systemowe okno wyboru folderu. Dodane foldery są widoczne w stopce jako etykiety. Kliknięcie etykiety usuwa folder z archiwum; same pliki na dysku zostają nietknięte.

## Skanowanie

Skanowanie rusza przy każdym starcie programu i po każdej zmianie listy folderów. Działa w osobnym wątku, więc w tym czasie można czytać dokumenty.

| Ustawienie | Wartość |
|:--|--:|
| Głębokość przeszukiwania | 9 poziomów |
| Limit plików | 20 000 |
| Rozszerzenia | `.md`, `.markdown`, `.mdown`, `.mkd`, `.mdx` |

Pomijane są katalogi, których nazwa zaczyna się od kropki, oraz katalogi o nazwach:

```text
node_modules  target  dist  build  out  vendor  venv  env
__pycache__  site-packages  bower_components  coverage
$RECYCLE.BIN  System Volume Information
```

> [!WARNING]
> Katalogi ukryte tylko atrybutem systemu Windows (na przykład `AppData`) nie są pomijane. Nie dodawaj całego katalogu domowego jako folderu archiwum.

## Wyszukiwanie

Zapytanie dzieli się na słowa. Wynik musi zawierać każde z nich w nazwie pliku albo w nazwie folderu. Wielkość liter i polskie znaki nie mają znaczenia: zapytanie `zrodla` znajdzie plik `źródła.md`, a `l` pasuje do `ł`.

Kolejność wyników:

1. najwyżej pliki, których nazwa zaczyna się od wpisanego słowa;
2. potem pliki, które mają to słowo w środku nazwy;
3. przy równej ocenie wyżej są pliki zmienione później.

Pliki `prd.md` i `roadmap.md` dostają niewielką premię. Bez zapytania okno pokazuje ostatnio otwierane dokumenty (do ośmiu) i właśnie te pliki.

### Klawiatura w oknie archiwum

- [x] strzałki w górę i w dół przesuwają zaznaczenie
- [x] `Enter` otwiera zaznaczony dokument
- [x] `Esc` lub kliknięcie poza oknem zamyka archiwum
- [ ] archiwum nie przeszukuje treści plików, tylko ich nazwy i foldery

## Gdzie leży indeks

Lista folderów i indeks archiwum są zapisywane w `%APPDATA%\pl.orphalese.ostrakon` jako `config.json` i `archive.json`. Indeks zawiera nazwę, ścieżkę, folder i datę zmiany każdego znalezionego pliku[^indeks].

[^indeks]: Treść dokumentów nie trafia do indeksu.

Powrót do [przeglądu](o-programie.md).
