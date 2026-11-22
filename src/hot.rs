//! Hot reload scoped to the functions that changed (`app.swap`, guide/swap.md).
//!
//! Saving a script normally reloads the whole app: every global is rebuilt and
//! `main()` runs again. When an edit only touched the bodies of top-level
//! functions, that's far more than needed: running just those definitions
//! again rebinds the globals to the new code, and everything else the app has
//! built up (its tables, entities, the camera) is left as it was.
//!
//! `plan` decides which it is from the two sources alone, with a small Lua
//! scanner (strings, comments, block depth) rather than the compiler, so it
//! works on a source that doesn't compile yet and never touches a VM. The
//! chunk it returns is the new source with every line outside the changed
//! functions blanked, so line numbers in errors still match the file.
//!
//! silt has a `hot-swap` feature of its own (`Lua::hotswap`). It isn't used
//! here: it compares the sources line by position (one inserted line makes
//! every later line "changed"), keeps a single root chunk where an app here is
//! many files in one VM, and on a root change it clears the globals, which
//! would take the engine's natives with them.

/// What an edit to one script needs.
#[derive(Debug, PartialEq)]
pub enum Plan {
    /// Nothing that runs changed (whitespace, comments): nothing to do.
    Same,
    /// Only these top-level functions changed. Run `chunk` and they're rebound.
    Fns { names: Vec<String>, chunk: String },
    /// The whole app has to reload; why.
    Root(&'static str),
}

#[derive(Clone, Debug, PartialEq)]
struct Tok {
    text: String,
    line: usize,
    /// a name or keyword, as opposed to a string, number or symbol
    word: bool,
}

/// A top-level `function name(...) ... end`.
struct Func {
    name: String,
    /// token range, `function` to its `end` inclusive
    from: usize,
    to: usize,
}

fn scan(src: &str) -> Vec<Tok> {
    let b: Vec<char> = src.chars().collect();
    let mut out = vec![];
    let (mut i, mut line) = (0, 1);
    // `[[`, `[=[`...: the level when a long bracket opens at `i`
    let long_open = |i: usize| -> Option<usize> {
        if b.get(i) != Some(&'[') {
            return None;
        }
        let mut j = i + 1;
        while b.get(j) == Some(&'=') {
            j += 1;
        }
        (b.get(j) == Some(&'[')).then(|| j - i - 1)
    };
    // the index just past the matching `]=*]`, counting lines
    let long_close = |mut i: usize, level: usize, line: &mut usize| -> usize {
        while i < b.len() {
            if b[i] == '\n' {
                *line += 1;
            }
            if b[i] == ']' {
                let mut j = i + 1;
                while b.get(j) == Some(&'=') {
                    j += 1;
                }
                if j - i - 1 == level && b.get(j) == Some(&']') {
                    return j + 1;
                }
            }
            i += 1;
        }
        i
    };
    while i < b.len() {
        let c = b[i];
        if c == '\n' {
            line += 1;
            i += 1;
        } else if c.is_whitespace() {
            i += 1;
        } else if c == '-' && b.get(i + 1) == Some(&'-') {
            match long_open(i + 2) {
                Some(level) => i = long_close(i + 4 + level, level, &mut line),
                None => {
                    while i < b.len() && b[i] != '\n' {
                        i += 1;
                    }
                }
            }
        } else if let Some(level) = long_open(i) {
            let (start, at) = (i, line);
            i = long_close(i + 2 + level, level, &mut line);
            out.push(Tok {
                text: b[start..i].iter().collect(),
                line: at,
                word: false,
            });
        } else if c == '"' || c == '\'' {
            let start = i;
            i += 1;
            while i < b.len() && b[i] != c && b[i] != '\n' {
                if b[i] == '\\' {
                    i += 1;
                }
                i += 1;
            }
            i = (i + 1).min(b.len());
            out.push(Tok {
                text: b[start..i].iter().collect(),
                line,
                word: false,
            });
        } else if c.is_alphabetic() || c == '_' {
            let start = i;
            while i < b.len() && (b[i].is_alphanumeric() || b[i] == '_') {
                i += 1;
            }
            out.push(Tok {
                text: b[start..i].iter().collect(),
                line,
                word: true,
            });
        } else if c.is_ascii_digit() {
            let start = i;
            while i < b.len()
                && (b[i].is_alphanumeric()
                    || b[i] == '_'
                    || (b[i] == '.' && b.get(i + 1).map_or(false, |n| n.is_ascii_digit())))
            {
                i += 1;
            }
            out.push(Tok {
                text: b[start..i].iter().collect(),
                line,
                word: false,
            });
        } else {
            out.push(Tok {
                text: c.to_string(),
                line,
                word: false,
            });
            i += 1;
        }
    }
    out
}

/// The top-level named functions, or None when the blocks don't balance.
fn funcs(toks: &[Tok]) -> Option<Vec<Func>> {
    let mut out = vec![];
    let mut depth = 0usize;
    let mut open: Option<(String, usize)> = None;
    for (i, t) in toks.iter().enumerate() {
        if !t.word {
            continue;
        }
        match t.text.as_str() {
            "function" | "if" | "do" | "repeat" => {
                if depth == 0 && t.text == "function" {
                    let local = i > 0 && toks[i - 1].text == "local";
                    let named = toks.get(i + 1).map_or(false, |n| n.word);
                    if named && !local {
                        let name: String = toks[i + 1..]
                            .iter()
                            .take_while(|n| n.text != "(")
                            .map(|n| n.text.as_str())
                            .collect();
                        open = Some((name, i));
                    }
                }
                depth += 1;
            }
            "end" | "until" => {
                depth = depth.checked_sub(1)?;
                if depth == 0 {
                    if let Some((name, from)) = open.take() {
                        out.push(Func { name, from, to: i });
                    }
                }
            }
            _ => {}
        }
    }
    (depth == 0).then_some(out)
}

/// Names declared `local` at the top level of the file. A function that uses
/// one closes over it, and a definition run again on its own wouldn't.
fn file_locals(toks: &[Tok], fns: &[Func]) -> Vec<String> {
    let mut out = vec![];
    let mut depth = 0usize;
    let mut i = 0;
    while i < toks.len() {
        if let Some(f) = fns.iter().find(|f| f.from == i) {
            i = f.to + 1;
            continue;
        }
        let t = &toks[i];
        if t.word {
            match t.text.as_str() {
                "function" | "if" | "do" | "repeat" => depth += 1,
                "end" | "until" => depth = depth.saturating_sub(1),
                "local" if depth == 0 => {
                    let mut j = i + 1;
                    if toks.get(j).map_or(false, |n| n.text == "function") {
                        j += 1;
                    }
                    while let Some(n) = toks.get(j) {
                        if n.word {
                            out.push(n.text.clone());
                            j += 1;
                            // `local a: number, b` (typed) or `local a, b`
                            while toks
                                .get(j)
                                .map_or(false, |n| n.text != "," && n.text != "=")
                                && toks.get(j).map_or(false, |n| n.line == t.line)
                            {
                                j += 1;
                            }
                        }
                        if toks.get(j).map_or(true, |n| n.text != ",") {
                            break;
                        }
                        j += 1;
                    }
                }
                _ => {}
            }
        }
        i += 1;
    }
    out
}

fn texts(toks: &[Tok]) -> Vec<&str> {
    toks.iter().map(|t| t.text.as_str()).collect()
}

/// The tokens outside every top-level function.
fn root_of<'a>(toks: &'a [Tok], fns: &[Func]) -> Vec<&'a Tok> {
    toks.iter()
        .enumerate()
        .filter(|(i, _)| !fns.iter().any(|f| f.from <= *i && *i <= f.to))
        .map(|(_, t)| t)
        .collect()
}

/// Work out what going from `old` to `new` takes.
pub fn plan(old: &str, new: &str) -> Plan {
    let (a, b) = (scan(old), scan(new));
    let (Some(fa), Some(fb)) = (funcs(&a), funcs(&b)) else {
        return Plan::Root("blocks don't balance");
    };
    let (ra, rb) = (root_of(&a, &fa), root_of(&b, &fb));
    if ra.len() != rb.len() || ra.iter().zip(&rb).any(|(x, y)| x.text != y.text) {
        return Plan::Root("top-level code changed");
    }
    for (i, f) in fb.iter().enumerate() {
        if fb[..i].iter().any(|g| g.name == f.name) {
            return Plan::Root("a function is defined twice");
        }
    }
    if fa.iter().any(|f| !fb.iter().any(|g| g.name == f.name)) {
        return Plan::Root("a function was removed or renamed");
    }
    let changed: Vec<&Func> = fb
        .iter()
        .filter(|f| match fa.iter().find(|g| g.name == f.name) {
            Some(g) => texts(&a[g.from..=g.to]) != texts(&b[f.from..=f.to]),
            None => true,
        })
        .collect();
    if changed.is_empty() {
        return Plan::Same;
    }
    let locals = file_locals(&b, &fb);
    for f in &changed {
        if b[f.from..=f.to]
            .iter()
            .any(|t| t.word && locals.contains(&t.text))
        {
            return Plan::Root("a changed function uses a file local");
        }
    }
    let spans: Vec<(usize, usize)> = changed
        .iter()
        .map(|f| (b[f.from].line, b[f.to].line))
        .collect();
    let inside = |line: usize| spans.iter().any(|(s, e)| *s <= line && line <= *e);
    // Something else sharing a line with a changed function would run too.
    if rb.iter().any(|t| inside(t.line))
        || fb
            .iter()
            .filter(|f| !changed.iter().any(|c| c.from == f.from))
            .any(|f| inside(b[f.from].line) || inside(b[f.to].line))
    {
        return Plan::Root("other code shares a line with a changed function");
    }
    let chunk: String = new
        .split('\n')
        .enumerate()
        .map(|(i, l)| if inside(i + 1) { l } else { "" })
        .collect::<Vec<_>>()
        .join("\n");
    Plan::Fns {
        names: changed.iter().map(|f| f.name.clone()).collect(),
        chunk,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const BASE: &str = "S = { n = 0 }\n\nfunction main()\n    S.n = 1\nend\n\nfunction loop()\n    if S.n > 0 then\n        S.n = S.n + 1\n    end\nend\n";

    fn names(p: Plan) -> Vec<String> {
        match p {
            Plan::Fns { names, .. } => names,
            other => panic!("wanted functions, got {other:?}"),
        }
    }

    #[test]
    fn a_body_edit_swaps_only_that_function() {
        let new = BASE.replace("S.n + 1", "S.n + 2");
        match plan(BASE, &new) {
            Plan::Fns { names, chunk } => {
                assert_eq!(names, ["loop"]);
                // main and the root are blanked, the lines kept where they were
                assert!(!chunk.contains("S = {") && !chunk.contains("function main"));
                assert_eq!(chunk.lines().nth(8), Some("        S.n = S.n + 2"));
                assert_eq!(chunk.matches('\n').count(), new.matches('\n').count());
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn inserted_lines_dont_spread_to_later_functions() {
        let new = BASE.replace("    S.n = 1\n", "    S.n = 1\n    S.m = 2\n    S.k = 3\n");
        assert_eq!(names(plan(BASE, &new)), ["main"]);
    }

    #[test]
    fn comments_and_spacing_are_no_change() {
        let new = BASE.replace("function loop()", "-- the tick\nfunction loop()   ");
        assert_eq!(plan(BASE, &new), Plan::Same);
    }

    #[test]
    fn root_edits_reload() {
        let new = BASE.replace("n = 0", "n = 5");
        assert_eq!(plan(BASE, &new), Plan::Root("top-level code changed"));
        let gone = BASE.replace("function main()", "function start()");
        assert!(matches!(plan(BASE, &gone), Plan::Root(_)));
        assert!(matches!(plan(BASE, "function main()\n"), Plan::Root(_)));
    }

    #[test]
    fn a_new_function_is_just_defined() {
        let new = format!("{BASE}\nfunction extra(a)\n    return a\nend\n");
        assert_eq!(names(plan(BASE, &new)), ["extra"]);
    }

    #[test]
    fn nested_functions_swap_their_outer_one() {
        let old = "function make_it()\n    local f = function(x)\n        return x\n    end\n    return f\nend\n";
        let new = old.replace("return x", "return x * 2");
        assert_eq!(names(plan(old, &new)), ["make_it"]);
    }

    #[test]
    fn keywords_in_strings_and_comments_dont_count() {
        let old = "function a()\n    local s = \"end if do\" -- end\n    local t = [[\nfunction end\n]]\n    return 1\nend\nfunction T.b()\n    return 2\nend\n";
        let new = old.replace("return 2", "return 3");
        assert_eq!(names(plan(old, &new)), ["T.b"]);
    }

    #[test]
    fn file_locals_force_a_reload() {
        let old = "local speed = 2\nfunction step(x)\n    return x + speed\nend\nfunction other()\n    return 1\nend\n";
        assert!(matches!(
            plan(old, &old.replace("x + speed", "x - speed")),
            Plan::Root("a changed function uses a file local")
        ));
        assert_eq!(
            names(plan(old, &old.replace("return 1", "return 4"))),
            ["other"]
        );
    }

    #[test]
    fn loops_and_repeats_balance() {
        let old = "function a()\n    for i = 1, 3 do\n        while i < 2 do\n            i = i + 1\n        end\n    end\n    repeat\n        local x = 1\n    until true\n    return 1\nend\n";
        assert_eq!(
            names(plan(old, &old.replace("return 1", "return 9"))),
            ["a"]
        );
    }
}
