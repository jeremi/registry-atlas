use semantic_asset_discovery_core::DiscoveryReport;
use std::fs;
use std::path::PathBuf;
use system_capability_discovery::{
    CapabilityIndex, CapabilityQuery, CapabilitySource, DiscoveryRunEnvelope, InformationNeed, Term,
};

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let args = Args::parse(std::env::args().skip(1))?;
    let mut query = if args.demo_social_protection {
        social_protection_query()
    } else {
        CapabilityQuery::new(args.query_id)
    };
    for need in args.needs {
        query = query.need(need);
    }

    let source = read_source(args.input)?;
    let index = CapabilityIndex::from_sources(vec![source]).map_err(|error| error.to_string())?;
    let result = index.search(query).map_err(|error| error.to_string())?;
    if args.pretty {
        println!(
            "{}",
            serde_json::to_string_pretty(&result).map_err(|error| error.to_string())?
        );
    } else {
        println!(
            "{}",
            serde_json::to_string(&result).map_err(|error| error.to_string())?
        );
    }
    Ok(())
}

fn read_source(input: Input) -> Result<CapabilitySource, String> {
    match input {
        Input::Envelope(path) => {
            let envelope: DiscoveryRunEnvelope = read_json(&path)?;
            Ok(CapabilitySource {
                id: path
                    .file_stem()
                    .and_then(|value| value.to_str())
                    .unwrap_or("envelope")
                    .to_string(),
                report: envelope.report.clone(),
                envelope: Some(envelope),
                mappings: Vec::new(),
                review: Vec::new(),
            })
        }
        Input::Report(path) => {
            let report: DiscoveryReport = read_json(&path)?;
            Ok(CapabilitySource {
                id: report.run_id.clone(),
                report,
                envelope: None,
                mappings: Vec::new(),
                review: Vec::new(),
            })
        }
    }
}

fn read_json<T: serde::de::DeserializeOwned>(path: &PathBuf) -> Result<T, String> {
    let text = fs::read_to_string(path)
        .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
    serde_json::from_str(&text)
        .map_err(|error| format!("failed to parse {} as JSON: {error}", path.display()))
}

fn social_protection_query() -> CapabilityQuery {
    CapabilityQuery::new("social_protection_program")
        .need(
            InformationNeed::new("farmer_status")
                .question("Is the person registered as a farmer?")
                .requires_any([Term::label("Farmer")]),
        )
        .need(
            InformationNeed::new("disability_status")
                .question("Does the person have a disability status?")
                .requires_all([
                    Term::label("Disabled Person"),
                    Term::field("disability_status"),
                ]),
        )
        .need(
            InformationNeed::new("school_attendance")
                .question("Are the person's children going to school?")
                .requires_any([Term::field("attendance_rate")]),
        )
}

#[derive(Debug)]
struct Args {
    input: Input,
    query_id: String,
    demo_social_protection: bool,
    pretty: bool,
    needs: Vec<InformationNeed>,
}

impl Args {
    fn parse<I>(items: I) -> Result<Self, String>
    where
        I: IntoIterator<Item = String>,
    {
        let mut args = items.into_iter();
        let mut input = None;
        let mut query_id = "capability_query".to_string();
        let mut demo_social_protection = false;
        let mut pretty = false;
        let mut needs = Vec::new();

        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--envelope" => {
                    input = Some(Input::Envelope(next_path(&mut args, "--envelope")?));
                }
                "--report" => {
                    input = Some(Input::Report(next_path(&mut args, "--report")?));
                }
                "--query-id" => {
                    query_id = next_value(&mut args, "--query-id")?;
                }
                "--need" => {
                    let id = next_value(&mut args, "--need id")?;
                    let kind = next_value(&mut args, "--need kind")?;
                    let value = next_value(&mut args, "--need value")?;
                    add_need_term(&mut needs, id, parse_term(&kind, value)?, TermMode::Any);
                }
                "--need-all" => {
                    let id = next_value(&mut args, "--need-all id")?;
                    let kind = next_value(&mut args, "--need-all kind")?;
                    let value = next_value(&mut args, "--need-all value")?;
                    add_need_term(&mut needs, id, parse_term(&kind, value)?, TermMode::All);
                }
                "--demo-social-protection" => {
                    demo_social_protection = true;
                }
                "--pretty" => {
                    pretty = true;
                }
                "-h" | "--help" => return Err(usage()),
                other => return Err(format!("unknown argument `{other}`\n\n{}", usage())),
            }
        }

        let Some(input) = input else {
            return Err(format!("missing --envelope or --report\n\n{}", usage()));
        };
        if !demo_social_protection && needs.is_empty() {
            return Err(format!(
                "provide --demo-social-protection or at least one --need/--need-all\n\n{}",
                usage()
            ));
        }

        Ok(Self {
            input,
            query_id,
            demo_social_protection,
            pretty,
            needs,
        })
    }
}

#[derive(Debug, Clone, Copy)]
enum TermMode {
    Any,
    All,
}

fn add_need_term(needs: &mut Vec<InformationNeed>, id: String, term: Term, mode: TermMode) {
    let need_index = needs.iter().position(|need| need.id == id);
    let need = if let Some(index) = need_index {
        &mut needs[index]
    } else {
        needs.push(InformationNeed::new(id));
        needs.last_mut().expect("pushed need")
    };
    match mode {
        TermMode::Any => need.requires_any.push(term),
        TermMode::All => need.requires_all.push(term),
    }
}

#[derive(Debug)]
enum Input {
    Envelope(PathBuf),
    Report(PathBuf),
}

fn parse_term(kind: &str, value: String) -> Result<Term, String> {
    match kind {
        "label" => Ok(Term::label(value)),
        "field" => Ok(Term::field(value)),
        "iri" => Ok(Term::iri(value)),
        other => Err(format!(
            "unsupported term kind `{other}`; expected label, field, or iri"
        )),
    }
}

fn next_path(args: &mut impl Iterator<Item = String>, name: &str) -> Result<PathBuf, String> {
    Ok(PathBuf::from(next_value(args, name)?))
}

fn next_value(args: &mut impl Iterator<Item = String>, name: &str) -> Result<String, String> {
    args.next()
        .ok_or_else(|| format!("missing value after {name}\n\n{}", usage()))
}

fn usage() -> String {
    "usage:
  system-capability-query --envelope <path> --demo-social-protection --pretty
  system-capability-query --report <path> --need <need-id> <label|field|iri> <value> [--need ...]
  system-capability-query --envelope <path> --need-all <need-id> <label|field|iri> <value> [--need-all ...]"
        .to_string()
}
