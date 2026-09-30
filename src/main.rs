use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::process;

const REQUIRED: &[&str] = &["name", "role", "purpose", "tone", "rules"];

#[derive(Debug, Clone, PartialEq, Eq)]
struct Persona {
    fields: BTreeMap<String, Vec<String>>,
}

impl Persona {
    fn get(&self, key: &str) -> Option<&str> {
        self.fields
            .get(key)
            .and_then(|values| values.first())
            .map(String::as_str)
    }

    fn values(&self, key: &str) -> &[String] {
        self.fields.get(key).map(Vec::as_slice).unwrap_or(&[])
    }
}

fn parse_persona(text: &str) -> Result<Persona, String> {
    let mut fields: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for (index, raw_line) in text.lines().enumerate() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let (key, value) = line
            .split_once(':')
            .ok_or_else(|| format!("line {}: expected 'key: value'", index + 1))?;
        let key = key.trim().to_ascii_lowercase();
        let value = value.trim().to_string();
        if key.is_empty() || !key.chars().all(|ch| ch.is_ascii_alphanumeric() || ch == '-') {
            return Err(format!("line {}: invalid key '{key}'", index + 1));
        }
        if value.is_empty() {
            return Err(format!("line {}: '{key}' cannot be empty", index + 1));
        }
        fields.entry(key).or_default().push(value);
    }
    if fields.is_empty() {
        return Err("persona has no fields".to_string());
    }
    Ok(Persona { fields })
}

fn missing_required(persona: &Persona) -> Vec<&'static str> {
    REQUIRED
        .iter()
        .copied()
        .filter(|key| persona.get(key).is_none())
        .collect()
}

fn validate(persona: &Persona) -> Result<(), String> {
    let missing = missing_required(persona);
    if !missing.is_empty() {
        return Err(format!("missing required fields: {}", missing.join(", ")));
    }
    if persona.values("rules").len() < 2 {
        return Err("provide at least two rules lines".to_string());
    }
    Ok(())
}

fn score(persona: &Persona) -> u8 {
    let mut total = 0u16;
    for key in REQUIRED {
        if persona.get(key).is_some() {
            total += 14;
        }
    }
    total += (persona.values("rules").len().min(6) as u16) * 3;
    if persona.get("examples").is_some() {
        total += 4;
    }
    if persona.get("voice").is_some() {
        total += 4;
    }
    if persona.get("safety").is_some() {
        total += 4;
    }
    total.min(100) as u8
}

fn render(persona: &Persona) -> Result<String, String> {
    validate(persona)?;
    let mut out = String::new();
    out.push_str(&format!("# {}\n\n", persona.get("name").unwrap()));
    out.push_str(&format!("**Role:** {}\n\n", persona.get("role").unwrap()));
    out.push_str(&format!("**Purpose:** {}\n\n", persona.get("purpose").unwrap()));
    out.push_str(&format!("**Tone:** {}\n\n", persona.get("tone").unwrap()));
    if let Some(voice) = persona.get("voice") {
        out.push_str(&format!("**Voice:** {voice}\n\n"));
    }
    out.push_str("## Rules\n\n");
    for rule in persona.values("rules") {
        out.push_str(&format!("- {rule}\n"));
    }
    if !persona.values("examples").is_empty() {
        out.push_str("\n## Examples\n\n");
        for example in persona.values("examples") {
            out.push_str(&format!("- {example}\n"));
        }
    }
    if let Some(safety) = persona.get("safety") {
        out.push_str(&format!("\n## Safety\n\n{safety}\n"));
    }
    Ok(out)
}

fn usage() {
    eprintln!(
        "personaforge — validate, score, and render .persona files\n\n\
         FORMAT:\n\
           name: warden\n\
           role: synthesis engine\n\
           purpose: turn chaos into coherence\n\
           tone: neon confidence with analog warmth\n\
           rules: Be direct\n\
           rules: Verify before claiming success\n\n\
         USAGE:\n\
           personaforge validate <file.persona>\n\
           personaforge score <file.persona>\n\
           personaforge render <file.persona>"
    );
}

fn load(path: &str) -> Result<Persona, String> {
    let text = fs::read_to_string(path).map_err(|e| format!("cannot read {path}: {e}"))?;
    parse_persona(&text)
}

fn run() -> Result<(), String> {
    let args: Vec<String> = env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("validate") => {
            let path = args.get(1).ok_or("validate requires a persona path")?;
            validate(&load(path)?)?;
            println!("VALID {path}");
            Ok(())
        }
        Some("score") => {
            let path = args.get(1).ok_or("score requires a persona path")?;
            let persona = load(path)?;
            println!("{} {}", score(&persona), path);
            Ok(())
        }
        Some("render") => {
            let path = args.get(1).ok_or("render requires a persona path")?;
            print!("{}", render(&load(path)?)?);
            Ok(())
        }
        Some("--help") | Some("-h") | None => {
            usage();
            Ok(())
        }
        Some(other) => Err(format!("unknown command {other}; try --help")),
    }
}

fn main() {
    if let Err(error) = run() {
        eprintln!("personaforge: {error}");
        process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const GOOD: &str = "name: warden\nrole: synthesis engine\npurpose: turn chaos into coherence\ntone: neon confidence\nrules: Be direct\nrules: Verify work\nexamples: Ship the fix\nvoice: analog warmth\nsafety: no secret leaks\n";

    #[test]
    fn parses_and_validates_good_persona() {
        let persona = parse_persona(GOOD).unwrap();
        validate(&persona).unwrap();
        assert_eq!(persona.get("name"), Some("warden"));
    }

    #[test]
    fn reports_missing_required_fields() {
        let persona = parse_persona("name: nobody\nrules: one\n").unwrap();
        let error = validate(&persona).unwrap_err();
        assert!(error.contains("role"));
        assert!(error.contains("purpose"));
    }

    #[test]
    fn score_rewards_complete_personas() {
        let sparse = parse_persona("name: a\nrole: b\npurpose: c\ntone: d\nrules: e\nrules: f\n").unwrap();
        let rich = parse_persona(GOOD).unwrap();
        assert!(score(&rich) > score(&sparse));
        assert!(score(&rich) <= 100);
    }

    #[test]
    fn renders_markdown() {
        let rendered = render(&parse_persona(GOOD).unwrap()).unwrap();
        assert!(rendered.contains("# warden"));
        assert!(rendered.contains("- Verify work"));
    }
}
