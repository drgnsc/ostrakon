# Elementy Markdown

Przegląd elementów, które Ostrakon renderuje. Program obsługuje składnię CommonMark z rozszerzeniami GitHub: tabele, przypisy, przekreślenia, listy zadań i alerty.

## Tekst

Zwykły akapit z **pogrubieniem**, *kursywą*, ~~przekreśleniem~~ i `kodem w linii`. Link do [sekcji o tabelach](#tabele) przewija ten dokument[^kotwica].

> Cytat jest wyróżniony kursywą i kreską po lewej stronie.

## Nagłówki

### Trzeci poziom

Nagłówki pierwszych trzech poziomów trafiają do spisu treści.

#### Czwarty poziom

Nagłówki od czwartego poziomu w dół są w tekście, ale nie w spisie.

## Listy

1. lista numerowana
2. z drugim punktem
   - i zagnieżdżoną listą punktowaną
   - z dwoma elementami

Lista zadań jest tylko do odczytu:

- [x] zadanie wykonane
- [ ] zadanie otwarte

## Tabele

| Wyrównanie | do lewej | do środka | do prawej |
|:--|:--|:-:|--:|
| przykład | a | b | 1 |
| przykład | aa | bb | 22 |
| przykład | aaa | bbb | 333 |

## Kod

Blok kodu pokazuje nazwę języka w prawym górnym rogu. Kolorowania składni nie ma.

```rust
fn main() {
    let path = std::env::args().nth(1);
    println!("{path:?}");
}
```

## Alerty

> [!NOTE]
> Notatka.

> [!TIP]
> Wskazówka.

> [!IMPORTANT]
> Ważne.

> [!WARNING]
> Uwaga.

> [!CAUTION]
> Ostrożnie.

## Rozwijane sekcje

<details>
<summary>Kliknij, żeby rozwinąć</summary>

Element `details` z osadzonego HTML jest dozwolony.

</details>

---

Powrót do [przeglądu](o-programie.md).

[^kotwica]: Kotwice działają też w linkach pisanych dla GitHuba, bo nagłówki dostają identyfikatory w tym samym formacie.
