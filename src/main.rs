use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs::File;
use std::io::prelude::*;
use std::path::Path;
use std::process::Command;
use std::time::{Duration, Instant};

fn main() -> std::io::Result<()> {
  //update_commit_file()?;

  let commits: Vec<String> = load_commit_file()?;
  //let (start, mut results) = fresh_start();
  let (start, mut results) = resume(&commits)?;

  let timer = Instant::now();
  println!("Warming up...");
  warm_up(&commits[0]);

  for (i, hash) in commits.iter().enumerate().skip(start) {
    println!(
      "{:.3E} secs: Processing commit {}/{}: {}",
      timer.elapsed().as_secs_f32(),
      i,
      commits.len(),
      hash
    );
    save_checkpoint(hash)?;
    process_commit(&mut results, hash)?;
    save_results(&results)?;
  }

  Ok(())
}

fn warm_up(sample_hash: &String) {
  let mut tmp = AllResults::new();

  process_commit(&mut tmp, sample_hash);
}

fn fresh_start() -> std::io::Result<(usize, AllResults)> {
  Ok((0, AllResults::new()))
}

fn resume(commits: &Vec<String>) -> std::io::Result<(usize, AllResults)> {
  Ok((load_checkpoint(commits)?, load_results()?))
}

fn save_checkpoint(hash: &String) -> std::io::Result<()> {
  let json = serde_json::to_string(hash)?;
  let mut f = File::create("commit_checkpoint.json")?;
  f.write_all(json.as_bytes())?;

  f.sync_all()?;
  Ok(())
}

fn load_checkpoint(commits: &Vec<String>) -> std::io::Result<usize> {
  let mut json: String = String::new();
  if Path::new("commit_checkpoint.json").exists() {
    let mut f = File::open("commit_checkpoint.json")?;
    f.read_to_string(&mut json)?;

    let hash: String = serde_json::from_str(json.as_str())?;

    Ok(
      commits
        .iter()
        .position(|s| *s == hash)
        .expect("Tried to load progress from non-existent commit."),
    )
  } else {
    Ok(0)
  }
}

fn process_commit(
  results: &mut AllResults, hash: &String,
) -> std::io::Result<()> {
  let bash =
    |s| Command::new("bash").current_dir("rav1e").arg("-c").arg(s).output();

  bash("cd rav1e; git stash; git stash drop")?;
  Command::new("git")
    .args(&["-C", "rav1e", "checkout", hash.as_str()])
    .output()?;
  Command::new("../compile-patches-rav1e/patch_commit.sh")
    .arg("rav1e")
    .output()?;
  //println!("{}", std::str::from_utf8(&tmp.stderr).expect("Invalid string"));

  let start = Instant::now();
  bash("cargo clean; cargo build")?;
  insert(&mut results.debug.full_build_times, hash, start.elapsed());

  let start = Instant::now();
  bash("touch src/lib.rs; cargo build")?;
  insert(&mut results.debug.partial_build_times, hash, start.elapsed());

  results.debug.binary_size.insert(
    hash.clone(),
    std::fs::metadata("rav1e/target/debug/rav1e")?.len(),
  );

  let start = Instant::now();
  bash("cargo clean; cargo build --release")?;
  insert(&mut results.release.full_build_times, hash, start.elapsed());

  let start = Instant::now();
  bash("touch src/lib.rs; cargo build --release")?;
  insert(&mut results.release.partial_build_times, hash, start.elapsed());

  results.release.binary_size.insert(
    hash.clone(),
    std::fs::metadata("rav1e/target/release/rav1e")?.len(),
  );

  Ok(())
}

fn save_results(output: &AllResults) -> std::io::Result<()> {
  let json = serde_json::to_string(output)?;
  let mut f = File::create("results.json")?;
  f.write_all(json.as_bytes())?;

  f.sync_all()?;
  Ok(())
}

fn load_results() -> std::io::Result<AllResults> {
  let mut json: String = String::new();
  if Path::new("results.json").exists() {
    let mut f = File::open("results.json")?;
    f.read_to_string(&mut json)?;

    let results: AllResults = serde_json::from_str(json.as_str())?;

    Ok(results)
  } else {
    Ok(AllResults::new())
  }
}

type BuildTimes = BTreeMap<String, Vec<Duration>>;
type BinarySizes = BTreeMap<String, u64>;

fn insert(map: &mut BuildTimes, hash: &String, d: Duration) {
  map.entry(hash.clone()).or_insert(vec![]).push(d);
}

#[derive(Serialize, Deserialize, Debug)]
struct ProfileResults {
  full_build_times: BuildTimes,
  partial_build_times: BuildTimes,
  binary_size: BinarySizes,
}
#[derive(Serialize, Deserialize, Debug)]
struct AllResults {
  debug: ProfileResults,
  release: ProfileResults,
}

impl ProfileResults {
  fn new() -> Self {
    Self {
      full_build_times: BuildTimes::new(),
      partial_build_times: BuildTimes::new(),
      binary_size: BinarySizes::new(),
    }
  }
}

impl AllResults {
  fn new() -> Self {
    Self { debug: ProfileResults::new(), release: ProfileResults::new() }
  }
}

fn load_commit_file() -> std::io::Result<Vec<String>> {
  let mut json: String = String::new();
  let mut f = File::open("CommitList.json")?;
  f.read_to_string(&mut json)?;

  let commits: Vec<String> = serde_json::from_str(json.as_str())?;

  Ok(commits)
}

fn update_commit_file() -> std::io::Result<()> {
  println!("A");
  let commits = create_commit_list()?;
  println!("B");
  let json = serde_json::to_string(&commits)?;
  let mut f = File::create("CommitList.json")?;
  f.write_all(json.as_bytes())?;

  f.sync_all()?;
  Ok(())
}

fn create_commit_list() -> std::io::Result<Vec<String>> {
  let output = Command::new("../compile-patches-rav1e/list_commits.sh")
    .arg("rav1e")
    .output()?;

  let mut commits: Vec<String> = Vec::new();
  let stdout: &str =
    std::str::from_utf8(&output.stdout).expect("Invalid string");

  for s in stdout.rsplit('\n').filter(|s: &&str| s.len() != 0) {
    commits.push(String::from(s));
  }

  Ok(commits)
}
