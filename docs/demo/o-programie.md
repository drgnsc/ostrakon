---
tytuł: Ostrakon, przegląd
rodzaj: dokument demonstracyjny
tagi:
  - markdown
  - podgląd
---

# Ostrakon

Ostrakon to podgląd plików Markdown na Windows. Program tylko czyta dokumenty: niczego w nich nie zapisuje. Blok `---` na początku tego pliku (front matter) jest wyświetlany jako karta z metadanymi nad treścią.

> [!NOTE]
> Ten folder zawiera kilka dokumentów demonstracyjnych. Pozostałe pliki z tego samego folderu widać w panelu „Obok” po lewej stronie.

## Otwieranie dokumentów

Dokument można otworzyć na kilka sposobów:

- dwuklikiem w pliku, jeśli Ostrakon jest programem domyślnym dla danego rozszerzenia;
- skrótem `Ctrl+O`, który otwiera systemowe okno wyboru pliku;
- przeciągając plik na okno programu;
- z archiwum (`Ctrl+P`), opisanego w dokumencie [Archiwum](archiwum.md).

Obsługiwane rozszerzenia to `.md`, `.markdown`, `.mdown`, `.mkd` i `.mdx`. Plik może mieć najwyżej 16 MB.

### Zmiany na dysku

Program co 0,8 sekundy sprawdza datę modyfikacji otwartego pliku. Gdy plik się zmieni, treść wczytuje się ponownie bez utraty pozycji przewijania.

## Skróty klawiszowe

| Klawisz | Działanie |
|:--|:--|
| `Ctrl+P` | archiwum: wyszukiwanie w dodanych folderach |
| `Ctrl+O` | otwórz plik |
| `Ctrl+B` lub `Ctrl+\` | pokaż lub ukryj panel boczny |
| `Ctrl+T` | jasny lub ciemny motyw |
| `Ctrl` `+` / `-` / `0`, `Ctrl` z kółkiem | rozmiar tekstu (od 80 do 150%) |
| `Alt+←`, boczny przycisk myszy | poprzedni dokument |
| `F5`, `Ctrl+R` | wczytaj ponownie |
| `Ctrl+W` | zamknij okno |

## Panel boczny

### Spis treści

Spis tworzy się z nagłówków pierwszych trzech poziomów i pojawia się, gdy dokument ma co najmniej dwa takie nagłówki. Kliknięcie przewija do nagłówka, a bieżąca sekcja jest zaznaczona kreską.

### Obok

Lista pozostałych plików Markdown z folderu otwartego dokumentu, posortowana alfabetycznie.

### Statystyki

Na dole panelu: liczba słów, szacowany czas czytania (200 słów na minutę) i data ostatniej zmiany pliku.

## Wygląd

Każdy dokument ma własny układ pęknięć i wyszczerbień karty. Układ jest wyliczany ze ścieżki pliku, więc przy kolejnym otwarciu wygląda tak samo.

```text
ścieżka pliku  ->  ziarno losowania  ->  kształt karty i pęknięcia
```

Motyw jasny i ciemny przełącza `Ctrl+T`. Bez wyboru użytkownika program stosuje motyw systemu.

## Pozostałe dokumenty

- [Archiwum](archiwum.md): wyszukiwanie w folderach z dokumentami
- [Obrazy i linki](obrazy-i-linki.md): jak program traktuje obrazy i odnośniki
- [Elementy Markdown](elementy-markdown.md): przegląd renderowanych elementów
