//! All disk access lives here. The frontend has no fs permission: it can only ask for
//! Markdown documents, and every path goes through these checks first.

use std::fs;
use std::path::{Component, Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use walkdir::WalkDir;

pub const MARKDOWN_EXTENSIONS: [&str; 5] = ["md", "markdown", "mdown", "mkd", "mdx"];
const MAX_DOCUMENT_BYTES: u64 = 16 * 1024 * 1024;
const MAX_IMAGE_BYTES: u64 = 32 * 1024 * 1024;
const MAX_ARCHIVE_ENTRIES: usize = 20_000;
const MAX_SCAN_DEPTH: usize = 9;
const SKIPPED_DIRS: [&str; 14] = [
    "node_modules", "target", "dist", "build", "out", "vendor", "venv", "env", "__pycache__",
    "site-packages", "bower_components", "coverage", "$RECYCLE.BIN", "System Volume Information",
];

pub fn has_markdown_extension(path: &Path) -> bool {
    extension_of(path).is_some_and(|e| MARKDOWN_EXTENSIONS.contains(&e.as_str()))
}

fn extension_of(path: &Path) -> Option<String> {
    path.extension().and_then(|e| e.to_str()).map(str::to_ascii_lowercase)
}

/// UNC paths are refused before they touch the filesystem: merely resolving one would make
/// Windows contact the remote host and offer it the user's NTLM credentials.
fn is_network_path(raw: &str) -> bool {
    let normalized = raw.replace('/', "\\");
    if let Some(rest) = normalized.strip_prefix(r"\\?\") {
        return rest.to_ascii_uppercase().starts_with("UNC\\");
    }
    normalized.starts_with(r"\\")
}

/// Canonical path of an existing local Markdown file, or a message fit for the user.
pub fn resolve_markdown(raw: &str) -> Result<PathBuf, String> {
    if is_network_path(raw) {
        return Err("Ostrakon nie otwiera plików z udziałów sieciowych.".into());
    }
    let path = Path::new(raw);
    if !has_markdown_extension(path) {
        return Err("To nie jest plik Markdown.".into());
    }
    let canonical = fs::canonicalize(path).map_err(|_| format!("Nie znaleziono pliku: {raw}"))?;
    if is_network_path(&canonical.to_string_lossy()) {
        return Err("Ostrakon nie otwiera plików z udziałów sieciowych.".into());
    }
    // Checked again: a symlink may point somewhere else than its own name suggests.
    if !has_markdown_extension(&canonical) {
        return Err("To nie jest plik Markdown.".into());
    }
    let metadata = fs::metadata(&canonical).map_err(|e| e.to_string())?;
    if !metadata.is_file() {
        return Err("To nie jest plik.".into());
    }
    if metadata.len() > MAX_DOCUMENT_BYTES {
        return Err("Plik jest większy niż 16 MB.".into());
    }
    Ok(canonical)
}

/// `href` from inside a document, resolved against that document's folder.
pub fn resolve_relative_markdown(document: &Path, href: &str) -> Result<PathBuf, String> {
    let clean = href.split(['#', '?']).next().unwrap_or("");
    let decoded = percent_encoding::percent_decode_str(clean).decode_utf8_lossy().to_string();
    if decoded.is_empty() || is_network_path(&decoded) {
        return Err("Nieprawidłowy odnośnik.".into());
    }
    let base = document.parent().ok_or("Nieprawidłowy odnośnik.")?;
    let joined = base.join(&decoded);
    resolve_markdown(&joined.to_string_lossy())
}

pub fn read_text(path: &Path) -> Result<String, String> {
    let bytes = fs::read(path).map_err(|e| format!("Nie udało się odczytać pliku: {e}"))?;
    Ok(decode(&bytes))
}

/// UTF-8 with or without BOM, plus UTF-16 with BOM (Windows tools still produce it).
fn decode(bytes: &[u8]) -> String {
    let utf16 = |to_u16: fn([u8; 2]) -> u16, data: &[u8]| {
        let units: Vec<u16> = data.as_chunks::<2>().0.iter().map(|c| to_u16(*c)).collect();
        String::from_utf16_lossy(&units)
    };
    match bytes {
        [0xEF, 0xBB, 0xBF, rest @ ..] => String::from_utf8_lossy(rest).into_owned(),
        [0xFF, 0xFE, rest @ ..] => utf16(u16::from_le_bytes, rest),
        [0xFE, 0xFF, rest @ ..] => utf16(u16::from_be_bytes, rest),
        _ => String::from_utf8_lossy(bytes).into_owned(),
    }
}

pub fn modified_ms(path: &Path) -> u64 {
    fs::metadata(path).and_then(|m| m.modified()).map(to_ms).unwrap_or(0)
}

fn to_ms(time: SystemTime) -> u64 {
    time.duration_since(UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

/// Nearest ancestor holding a `.git`, used as the project name and as the image sandbox.
/// A repository at the user's home folder is not a project: it would widen the sandbox to everything.
pub fn project_root(document: &Path) -> Option<PathBuf> {
    let home = std::env::var_os("USERPROFILE").and_then(|h| fs::canonicalize(h).ok());
    document
        .ancestors()
        .skip(1)
        .take_while(|dir| home.as_deref() != Some(*dir))
        .find(|dir| dir.join(".git").exists())
        .map(Path::to_path_buf)
}

/// Breadcrumb from the project (or the last two folders) down to the document's folder.
pub fn crumbs(document: &Path) -> Vec<String> {
    let parent = document.parent().unwrap_or(document);
    let names = |path: &Path| -> Vec<String> {
        path.components()
            .filter_map(|c| match c {
                Component::Normal(n) => Some(n.to_string_lossy().into_owned()),
                _ => None,
            })
            .collect()
    };
    if let Some(root) = project_root(document) {
        let mut out = vec![root.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()];
        if let Ok(rel) = parent.strip_prefix(&root) {
            out.extend(names(rel));
        }
        return out;
    }
    let all = names(parent);
    all[all.len().saturating_sub(2)..].to_vec()
}

#[derive(Serialize, Clone)]
pub struct Sibling {
    pub name: String,
    pub path: String,
    pub current: bool,
}

pub fn siblings(document: &Path) -> Vec<Sibling> {
    let Some(parent) = document.parent() else { return Vec::new() };
    let Ok(entries) = fs::read_dir(parent) else { return Vec::new() };
    let mut out: Vec<Sibling> = entries
        .flatten()
        .filter(|e| e.file_type().is_ok_and(|t| t.is_file()) && has_markdown_extension(&e.path()))
        .map(|e| Sibling {
            name: e.file_name().to_string_lossy().into_owned(),
            path: display_path(&e.path()),
            current: e.path() == document,
        })
        .take(200)
        .collect();
    out.sort_by_key(|s| s.name.to_lowercase());
    out
}

/// Drops the `\\?\` prefix std::fs::canonicalize adds on Windows.
pub fn display_path(path: &Path) -> String {
    let text = path.to_string_lossy();
    text.strip_prefix(r"\\?\").unwrap_or(&text).to_string()
}

#[derive(Serialize, Deserialize, Clone)]
pub struct ArchiveEntry {
    pub name: String,
    pub path: String,
    /// Folder relative to the scanned root, with the root's own name first.
    pub place: String,
    pub modified: u64,
}

fn is_skipped_dir(name: &str) -> bool {
    name.starts_with('.') || SKIPPED_DIRS.iter().any(|s| s.eq_ignore_ascii_case(name))
}

pub fn scan_roots(roots: &[String]) -> Vec<ArchiveEntry> {
    let mut out = Vec::new();
    for root in roots {
        if is_network_path(root) {
            continue;
        }
        let root_path = Path::new(root);
        let root_name = root_path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let walker = WalkDir::new(root_path).max_depth(MAX_SCAN_DEPTH).into_iter().filter_entry(|e| {
            e.depth() == 0 || !e.file_type().is_dir() || !is_skipped_dir(&e.file_name().to_string_lossy())
        });
        for entry in walker.flatten() {
            if out.len() >= MAX_ARCHIVE_ENTRIES {
                return out;
            }
            if !entry.file_type().is_file() || !has_markdown_extension(entry.path()) {
                continue;
            }
            let parent = entry.path().parent().unwrap_or(root_path);
            let rel = parent.strip_prefix(root_path).unwrap_or(parent);
            let place = std::iter::once(root_name.clone())
                .chain(rel.components().map(|c| c.as_os_str().to_string_lossy().into_owned()))
                .collect::<Vec<_>>()
                .join(" / ");
            out.push(ArchiveEntry {
                name: entry.file_name().to_string_lossy().into_owned(),
                path: display_path(entry.path()),
                place,
                modified: entry.metadata().ok().and_then(|m| m.modified().ok()).map(to_ms).unwrap_or(0),
            });
        }
    }
    out
}

fn image_mime(extension: &str) -> Option<&'static str> {
    Some(match extension {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "avif" => "image/avif",
        "bmp" => "image/bmp",
        // Rendered through <img>, where SVG scripts never execute.
        "svg" => "image/svg+xml",
        _ => return None,
    })
}

/// An image referenced by `document`: relative, inside the project, and really an image.
pub fn read_project_image(document: &Path, relative: &str) -> Result<(Vec<u8>, &'static str), String> {
    let decoded = percent_encoding::percent_decode_str(relative).decode_utf8_lossy().to_string();
    let clean = decoded.split(['#', '?']).next().unwrap_or("");
    if is_network_path(clean) || Path::new(clean).is_absolute() {
        return Err("absolute path".into());
    }
    let base = document.parent().ok_or("no parent")?;
    let root = project_root(document).unwrap_or_else(|| base.to_path_buf());
    let canonical = fs::canonicalize(base.join(clean)).map_err(|e| e.to_string())?;
    if !canonical.starts_with(&root) {
        return Err("outside project".into());
    }
    let mime = extension_of(&canonical).and_then(|e| image_mime(&e)).ok_or("not an image")?;
    let metadata = fs::metadata(&canonical).map_err(|e| e.to_string())?;
    if !metadata.is_file() || metadata.len() > MAX_IMAGE_BYTES {
        return Err("not a usable file".into());
    }
    Ok((fs::read(&canonical).map_err(|e| e.to_string())?, mime))
}

#[derive(Serialize, Deserialize, Clone, Default)]
pub struct Config {
    pub roots: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("ostrakon-test-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        fs::canonicalize(dir).unwrap()
    }

    #[test]
    fn detects_network_paths() {
        for p in [r"\\server\share\a.md", "//server/share/a.md", r"\\?\UNC\server\share\a.md", r"\\?\unc\s\a.md"] {
            assert!(is_network_path(p), "{p}");
        }
        for p in [r"C:\a.md", r"\\?\C:\a.md", "a.md", r"..\a.md"] {
            assert!(!is_network_path(p), "{p}");
        }
    }

    #[test]
    fn refuses_network_and_non_markdown_paths() {
        assert!(resolve_markdown(r"\\evil\share\a.md").unwrap_err().contains("sieciowych"));
        let dir = temp_dir("ext");
        let exe = dir.join("a.exe");
        fs::write(&exe, "x").unwrap();
        assert!(resolve_markdown(&exe.to_string_lossy()).is_err());
        let env = dir.join(".env");
        fs::write(&env, "x").unwrap();
        assert!(resolve_markdown(&env.to_string_lossy()).is_err());
    }

    #[test]
    fn refuses_oversized_documents() {
        let dir = temp_dir("big");
        let big = dir.join("big.md");
        let file = fs::File::create(&big).unwrap();
        file.set_len(MAX_DOCUMENT_BYTES + 1).unwrap();
        assert!(resolve_markdown(&big.to_string_lossy()).unwrap_err().contains("16 MB"));
    }

    #[test]
    fn relative_links_resolve_only_to_markdown() {
        let dir = temp_dir("rel");
        let doc = dir.join("a.md");
        fs::write(&doc, "x").unwrap();
        fs::write(dir.join("b c.md"), "y").unwrap();
        fs::write(dir.join("evil.bat"), "y").unwrap();
        assert!(resolve_relative_markdown(&doc, "b%20c.md#section").unwrap().ends_with("b c.md"));
        assert!(resolve_relative_markdown(&doc, "evil.bat").is_err());
        assert!(resolve_relative_markdown(&doc, r"\\evil\x.md").is_err());
        assert!(resolve_relative_markdown(&doc, "%5C%5Cevil%5Cx.md").is_err());
    }

    #[test]
    fn decodes_bom_variants() {
        assert_eq!(decode(&[0xEF, 0xBB, 0xBF, b'a']), "a");
        assert_eq!(decode(&[0xFF, 0xFE, b'a', 0]), "a");
        assert_eq!(decode(&[0xFE, 0xFF, 0, b'a']), "a");
        assert_eq!(decode("żółć".as_bytes()), "żółć");
    }

    #[test]
    fn images_stay_inside_the_document_folder() {
        let root = temp_dir("img");
        let sub = root.join("sub");
        fs::create_dir_all(&sub).unwrap();
        let doc = sub.join("a.md");
        fs::write(&doc, "x").unwrap();
        fs::write(sub.join("ok.png"), "p").unwrap();
        fs::write(sub.join("secret.txt"), "s").unwrap();
        fs::write(root.join("outside.png"), "o").unwrap();

        assert_eq!(read_project_image(&doc, "ok.png").unwrap().1, "image/png");
        assert!(read_project_image(&doc, "secret.txt").is_err());
        assert!(read_project_image(&doc, "../outside.png").is_err());
        assert!(read_project_image(&doc, r"\\evil\x.png").is_err());
        assert!(read_project_image(&doc, r"C:\Windows\x.png").is_err());
    }

    #[test]
    fn images_may_use_the_whole_git_project() {
        let root = temp_dir("git");
        fs::create_dir_all(root.join(".git")).unwrap();
        let sub = root.join("docs");
        fs::create_dir_all(&sub).unwrap();
        let doc = sub.join("a.md");
        fs::write(&doc, "x").unwrap();
        fs::write(root.join("shot.png"), "p").unwrap();
        assert!(read_project_image(&doc, "../shot.png").is_ok());
    }

    #[test]
    fn scan_skips_heavy_and_hidden_folders() {
        let root = temp_dir("scan");
        for dir in ["p/context/foundation", "p/node_modules/x", "p/.git/x", "p/target/x"] {
            fs::create_dir_all(root.join(dir)).unwrap();
        }
        fs::write(root.join("p/context/foundation/prd.md"), "x").unwrap();
        fs::write(root.join("p/node_modules/x/README.md"), "x").unwrap();
        fs::write(root.join("p/.git/x/a.md"), "x").unwrap();
        fs::write(root.join("p/target/x/a.md"), "x").unwrap();
        fs::write(root.join("p/not-markdown.txt"), "x").unwrap();

        let found = scan_roots(&[display_path(&root)]);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].name, "prd.md");
        assert!(found[0].place.ends_with("p / context / foundation"));
    }
}
