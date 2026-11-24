// Build steps: `.rhai` scripts run between assembly and packing.
//
// The contract is docs/rewind-rhai-contract.md in the IDE repo. Three properties matter:
//   · every path is project-root relative and a path escaping the root is an error, not a
//     fallback — a step that writes `../x` would edit files outside the project;
//   · the engine is bounded twice (operations and wall clock) so a bad script cannot hang
//     the build; the two limits land differently, see below;
//   · a failure names the step number, the script path and the engine message, because the
//     reader needs to know which step died before they can look at the script.

use rhai::{Dynamic, Engine, EvalAltResult};
use std::cell::RefCell;
use std::path::{Component, Path, PathBuf};
use std::rc::Rc;
use std::time::{Duration, Instant};

use crate::support::config::BuildStep;

/// Operations per step. A start value, not a measured optimum.
const MAX_OPERATIONS: u64 = 10_000_000;
/// Wall clock per step, same caveat.
const MAX_DURATION: Duration = Duration::from_secs(30);
/// Test hook: the wall clock the engine enforces. A test cannot spend 30 real seconds, so the
/// limit is a value rather than a constant in the code path.
static CLOCK_LIMIT: std::sync::atomic::AtomicU64 =
    std::sync::atomic::AtomicU64::new(MAX_DURATION.as_secs());

fn clock_limit() -> Duration {
    Duration::from_secs(CLOCK_LIMIT.load(std::sync::atomic::Ordering::Relaxed))
}
/// The progress callback runs on every operation; comparing clocks that often costs more
/// than the check is worth, so only every this many operations.
const CLOCK_CHECK_EVERY: u64 = 4096;

pub(crate) struct StepContext<'a> {
    pub project_dir: &'a Path,
    pub out_dir: &'a Path,
    pub module: &'a str,
    pub variant: &'a str,
}

/// Run only the steps declared for one phase. `pre` runs before the ArkTS compile so a step can
/// generate a source file; `post` runs after assembly so a step can adjust what gets packed.
pub(crate) fn run_phase(
    ctx: &StepContext,
    steps: &[BuildStep],
    phase: &str,
) -> Result<Vec<String>, String> {
    let mut emitted: Vec<String> = Vec::new();
    for (index, step) in steps.iter().enumerate() {
        if step.phase != phase {
            continue;
        }
        match step.kind.as_str() {
            "run" => emitted.extend(run_one(ctx, step, index)?),
            other => {
                return Err(format!(
                    "step {}: unknown kind `{}` (registered: run)",
                    index + 1,
                    other
                ))
            }
        }
    }
    Ok(emitted)
}

fn run_one(ctx: &StepContext, step: &BuildStep, index: usize) -> Result<Vec<String>, String> {
    let script_path = resolve(ctx.project_dir, &step.path)
        .map_err(|e| format!("step {}: {} — {}", index + 1, step.path, e))?;
    let source = std::fs::read_to_string(&script_path)
        .map_err(|e| format!("step {}: {} — {}", index + 1, step.path, e))?;

    let mut engine = Engine::new();
    engine.set_max_operations(MAX_OPERATIONS);
    // The stock depth caps (32 nested / 32 function) reject ordinary code: a scanner that
    // indexes into a string inside a loop hits "Expression exceeds maximum complexity" without
    // doing anything wrong. Depth is a parser guard, not a budget — the real bounds for a
    // runaway script are the operation count and the wall clock above.
    engine.set_max_expr_depths(128, 128);
    let started = Instant::now();
    let limit = clock_limit();
    // rhai reports every abort as `Script terminated`, so the reason is recorded here and
    // spliced into the message afterwards; without it the build error cannot say which of the
    // two limits fired, and the contract requires it to.
    let timed_out = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let flag = timed_out.clone();
    engine.on_progress(move |count: u64| {
        if count % CLOCK_CHECK_EVERY == 0 && started.elapsed() > limit {
            flag.store(true, std::sync::atomic::Ordering::Relaxed);
            return Some(Dynamic::from("wall clock limit exceeded"));
        }
        None
    });
    let emitted = register_functions(&mut engine, ctx);

    let outcome = engine.run(&source).map(|_| emitted.borrow().clone());
    outcome.map_err(|e| {
        if timed_out.load(std::sync::atomic::Ordering::Relaxed) {
            format!(
                "step {}: {} — wall clock limit ({}s) exceeded",
                index + 1,
                step.path,
                limit.as_secs()
            )
        } else {
            format!("step {}: {} — {}", index + 1, step.path, e)
        }
    })
}

/// Resolve a project-root-relative path, refusing anything that leaves the root.
fn resolve(root: &Path, rel: &str) -> Result<PathBuf, String> {
    let candidate = Path::new(rel);
    if candidate.is_absolute() {
        return Err("path must be relative to the project root".to_string());
    }
    let mut depth = 0i32;
    for component in candidate.components() {
        match component {
            Component::ParentDir => {
                depth -= 1;
                if depth < 0 {
                    return Err("path escapes the project root".to_string());
                }
            }
            Component::Normal(_) => depth += 1,
            _ => {}
        }
    }
    Ok(root.join(candidate))
}

fn register_functions(engine: &mut Engine, ctx: &StepContext) -> Rc<RefCell<Vec<String>>> {
    let project_dir = ctx.project_dir.to_path_buf();
    let out_dir = ctx.out_dir.to_path_buf();
    let module = ctx.module.to_string();
    let variant = ctx.variant.to_string();

    // Build context, read only. The contract promises absolute paths: a script that prints
    // or joins these gets a usable path only if the base is not `.`.
    let absolute_root = absolutize(&project_dir);
    engine.register_fn("project_dir", move || absolute_root.clone());
    let absolute_out = absolutize(&out_dir);
    engine.register_fn("out_dir", move || absolute_out.clone());
    let module_name = module.clone();
    // The contract says `module()`, but `module` is a reserved keyword in rhai and cannot
    // be registered as a function. The name carries the difference instead.
    engine.register_fn("module_name", move || module_name.clone());
    let variant_name = variant.clone();
    engine.register_fn("variant", move || variant_name.clone());

    // File operations, all project-root relative.
    let root = project_dir.clone();
    engine.register_fn(
        "read",
        move |path: &str| -> Result<String, Box<EvalAltResult>> {
            let full = resolve(&root, path).map_err(err)?;
            std::fs::read_to_string(&full).map_err(|e| err(format!("{}: {}", path, e)))
        },
    );
    let root = project_dir.clone();
    engine.register_fn(
        "write",
        move |path: &str, text: &str| -> Result<i64, Box<EvalAltResult>> {
            let full = resolve(&root, path).map_err(err)?;
            if let Some(parent) = full.parent() {
                std::fs::create_dir_all(parent).map_err(|e| err(format!("{}: {}", path, e)))?;
            }
            std::fs::write(&full, text).map_err(|e| err(format!("{}: {}", path, e)))?;
            Ok(text.len() as i64)
        },
    );
    let root = project_dir.clone();
    engine.register_fn("exists", move |path: &str| -> bool {
        resolve(&root, path).map(|p| p.exists()).unwrap_or(false)
    });
    // A script that walks a tree has to tell a file from a directory before it recurses;
    // `exists` answers neither question, and `list` errors on a file.
    let root = project_dir.clone();
    engine.register_fn("is_dir", move |path: &str| -> bool {
        resolve(&root, path).map(|p| p.is_dir()).unwrap_or(false)
    });
    let root = project_dir.clone();
    engine.register_fn(
        "list",
        move |path: &str| -> Result<rhai::Array, Box<EvalAltResult>> {
            let full = resolve(&root, path).map_err(err)?;
            let mut names: Vec<String> = Vec::new();
            for entry in std::fs::read_dir(&full).map_err(|e| err(format!("{}: {}", path, e)))? {
                let entry = entry.map_err(|e| err(format!("{}: {}", path, e)))?;
                names.push(entry.file_name().to_string_lossy().to_string());
            }
            names.sort();
            // A plain Vec<String> is not a rhai array: the engine prints it as an opaque Rust
            // value and `for name in list(..)` fails. Convert element by element.
            Ok(names.into_iter().map(Dynamic::from).collect())
        },
    );
    let root = project_dir.clone();
    engine.register_fn(
        "mkdir",
        move |path: &str| -> Result<bool, Box<EvalAltResult>> {
            let full = resolve(&root, path).map_err(err)?;
            if full.exists() {
                return Ok(false);
            }
            std::fs::create_dir_all(&full).map_err(|e| err(format!("{}: {}", path, e)))?;
            Ok(true)
        },
    );
    let root = project_dir.clone();
    engine.register_fn(
        "remove",
        move |path: &str| -> Result<bool, Box<EvalAltResult>> {
            let full = resolve(&root, path).map_err(err)?;
            if !full.exists() {
                return Ok(false);
            }
            if full.is_dir() {
                std::fs::remove_dir_all(&full).map_err(|e| err(format!("{}: {}", path, e)))?;
            } else {
                std::fs::remove_file(&full).map_err(|e| err(format!("{}: {}", path, e)))?;
            }
            Ok(true)
        },
    );

    // Path helpers. These stay lexical: they never touch the filesystem.
    engine.register_fn("join", |a: &str, b: &str| -> String {
        let joined: PathBuf = Path::new(a).join(b);
        joined.to_string_lossy().to_string()
    });
    engine.register_fn("normalize", |path: &str| -> String {
        let mut parts: Vec<String> = Vec::new();
        for component in Path::new(path).components() {
            match component {
                Component::ParentDir => {
                    parts.pop();
                }
                Component::CurDir => {}
                Component::Normal(part) => parts.push(part.to_string_lossy().to_string()),
                _ => {}
            }
        }
        parts.join("/")
    });

    // Logging and failure. No process execution: the sandbox forbids execve, so a `sh`
    // function would work on the host and fail on the device with the same hvolt.toml.
    engine.register_fn("info", |text: &str| {
        println!("    {}", text);
    });
    engine.register_fn("warn", |text: &str| {
        eprintln!("    warning: {}", text);
    });
    engine.register_fn("fail", |text: &str| -> Result<(), Box<EvalAltResult>> {
        Err(err(text.to_string()))
    });

    // zip(out, entries)：把若干项目内文件打成一个归档，写到 out_dir 下。
    //
    // 这是「内置源码可以自举」缺的那一环：pack-self 现在靠宿主机的 rsync 与 zip 二进制，
    // 设备上没有它们。归档路径相对 out_dir，条目相对 project_dir。
    let zip_root = project_dir.clone();
    let zip_out = out_dir.clone();
    engine.register_fn(
        "zip",
        move |out: &str, entries: rhai::Array| -> Result<i64, Box<EvalAltResult>> {
            let target = resolve(&zip_out, out).map_err(err)?;
            if let Some(parent) = target.parent() {
                std::fs::create_dir_all(parent).map_err(|e| err(format!("{}: {}", out, e)))?;
            }
            let file =
                std::fs::File::create(&target).map_err(|e| err(format!("{}: {}", out, e)))?;
            let mut writer = zip::ZipWriter::new(file);
            let options = zip::write::SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Deflated);
            let mut written: i64 = 0;
            for entry in entries {
                let name = entry.to_string();
                let full = resolve(&zip_root, &name).map_err(err)?;
                let data = std::fs::read(&full).map_err(|e| err(format!("{}: {}", name, e)))?;
                writer
                    .start_file(name.clone(), options)
                    .map_err(|e| err(format!("{}: {}", name, e)))?;
                use std::io::Write;
                writer
                    .write_all(&data)
                    .map_err(|e| err(format!("{}: {}", name, e)))?;
                written += 1;
            }
            writer
                .finish()
                .map_err(|e| err(format!("{}: {}", out, e)))?;
            Ok(written)
        },
    );

    // Emitted artefacts: the path is relative to out_dir. The list goes back to the caller so
    // the packing stage can intersect it with what the directory holds; an empty list means
    // the step claimed nothing and the whole directory is packed as before.
    let emitted: Rc<RefCell<Vec<String>>> = Rc::new(RefCell::new(Vec::new()));
    let sink = emitted.clone();
    engine.register_fn("emit", move |path: &str| {
        println!("    emit {}", path);
        sink.borrow_mut().push(path.to_string());
    });
    emitted
}

/// Absolute form of `path`. `canonicalize` fails on a directory that does not exist yet,
/// which is normal for `out_dir` on a first build, so fall back to the process directory.
fn absolutize(path: &Path) -> String {
    match std::fs::canonicalize(path) {
        Ok(full) => full.to_string_lossy().to_string(),
        Err(_) => {
            let base = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
            base.join(path).to_string_lossy().to_string()
        }
    }
}

fn err(message: String) -> Box<EvalAltResult> {
    Box::new(EvalAltResult::ErrorRuntime(
        Dynamic::from(message),
        rhai::Position::NONE,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sandbox(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("hvolt-step-{}-{}", name, std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("build")).unwrap();
        std::fs::create_dir_all(dir.join("scripts")).unwrap();
        dir
    }

    fn run_script(root: &Path, body: &str) -> Result<Vec<String>, String> {
        std::fs::write(root.join("scripts/t.rhai"), body).unwrap();
        let out = root.join("build");
        let ctx = StepContext {
            project_dir: root,
            out_dir: &out,
            module: "entry",
            variant: "default",
        };
        let steps = vec![BuildStep {
            kind: "run".to_string(),
            path: "scripts/t.rhai".to_string(),
            phase: "post".to_string(),
        }];
        run_phase(&ctx, &steps, "post")
    }

    #[test]
    fn write_then_read_round_trips() {
        let root = sandbox("rw");
        let ok = run_script(
            &root,
            "let n = write(\"build/out.txt\", \"hi\"); if n != 2 { fail(\"bad length\"); } if read(\"build/out.txt\") != \"hi\" { fail(\"bad read\"); } emit(\"build/out.txt\");",
        );
        assert!(ok.is_ok(), "{:?}", ok);
        assert_eq!(ok.unwrap(), vec!["build/out.txt".to_string()]);
        assert_eq!(
            std::fs::read_to_string(root.join("build/out.txt")).unwrap(),
            "hi"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn failing_script_names_the_step() {
        let root = sandbox("fail");
        let err = run_script(&root, "fail(\"nope\")").unwrap_err();
        assert!(err.starts_with("step 1: scripts/t.rhai — "), "{}", err);
        assert!(err.contains("nope"), "{}", err);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn path_escaping_the_root_is_refused() {
        let root = sandbox("escape");
        let err = run_script(&root, "read(\"../outside.txt\")").unwrap_err();
        assert!(err.contains("escapes the project root"), "{}", err);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn ordinary_nested_code_is_accepted() {
        // A scanner that indexes into a string inside a loop used to trip the stock depth
        // caps; this pins the raised limit so the example script keeps working.
        let root = sandbox("depth");
        let body = r#"
            let text = "a//b";
            let out = "";
            let i = 0;
            while i < text.len() {
                let c = text[i];
                if c == "/" {
                    let next = i + 1;
                    if next < text.len() {
                        if text[next] == "/" { break; }
                    }
                }
                out += c;
                i += 1;
            }
            if out != "a" { fail("got " + out); }
        "#;
        assert!(run_script(&root, body).is_ok());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn wall_clock_limit_stops_a_script() {
        let root = sandbox("clock");
        // A busy loop long enough to reach the first clock check.
        let body = "let i = 0; while i < 200000 { i += 1; }";
        CLOCK_LIMIT.store(0, std::sync::atomic::Ordering::Relaxed);
        let err = run_script(&root, body).unwrap_err();
        CLOCK_LIMIT.store(MAX_DURATION.as_secs(), std::sync::atomic::Ordering::Relaxed);
        // The limit is named after the fact: the engine's own message is the generic
        // `Script terminated`, so the reason comes from the flag the callback set.
        assert!(err.contains("wall clock limit"), "{}", err);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn unknown_kind_is_named() {
        let root = sandbox("kind");
        let out = root.join("build");
        let ctx = StepContext {
            project_dir: &root,
            out_dir: &out,
            module: "entry",
            variant: "default",
        };
        let steps = vec![BuildStep {
            kind: "copy".to_string(),
            path: "x".to_string(),
            phase: "post".to_string(),
        }];
        let err = run_phase(&ctx, &steps, "post").unwrap_err();
        assert!(err.contains("unknown kind `copy`"), "{}", err);
        let _ = std::fs::remove_dir_all(&root);
    }
}
