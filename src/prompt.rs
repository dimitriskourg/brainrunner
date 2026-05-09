pub fn build_prompt(number: u64, title: &str, body: &str, comments: &[&str]) -> String {
    let mut issue_text = format!("# Issue #{number}: {title}\n\n{body}");
    if !comments.is_empty() {
        issue_text.push_str("\n\n## Comments\n");
        for c in comments {
            issue_text.push_str("\n---\n");
            issue_text.push_str(c);
        }
    }

    format!(
        "{issue_text}\n\n\
        ## Instructions\n\
        Read the codebase, then implement the issue above.\n\
        Commit your changes in small, logical increments.\n\
        After each change, run any available type-checking and linting commands you find \
        in the project (e.g. from Makefile, package.json, Cargo.toml, or similar).\n\
        Do not push the branch or submit changes upstream — lifecycle transitions are handled externally.\n\
        When the implementation is complete, emit the following sigil on its own line:\n\
        <promise>COMPLETE</promise>"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prompt_contains_title() {
        let prompt = build_prompt(42, "Fix the flux capacitor", "body", &[]);
        assert!(prompt.contains("Fix the flux capacitor"), "got: {prompt}");
    }

    #[test]
    fn prompt_contains_body() {
        let prompt = build_prompt(42, "title", "Detailed description of the problem", &[]);
        assert!(
            prompt.contains("Detailed description of the problem"),
            "got: {prompt}"
        );
    }

    #[test]
    fn prompt_contains_all_comments() {
        let prompt = build_prompt(7, "title", "body", &["first comment", "second comment"]);
        assert!(prompt.contains("first comment"), "got: {prompt}");
        assert!(prompt.contains("second comment"), "got: {prompt}");
    }

    #[test]
    fn prompt_contains_completion_sigil_instruction() {
        let prompt = build_prompt(1, "title", "body", &[]);
        assert!(
            prompt.contains("<promise>COMPLETE</promise>"),
            "got: {prompt}"
        );
    }

    #[test]
    fn prompt_instructs_to_run_available_typecheck_and_lint() {
        let prompt = build_prompt(1, "title", "body", &[]);
        let lower = prompt.to_lowercase();
        assert!(
            lower.contains("type-check")
                || lower.contains("typecheck")
                || lower.contains("type check"),
            "expected type-check instruction, got: {prompt}"
        );
        assert!(
            lower.contains("lint"),
            "expected lint instruction, got: {prompt}"
        );
        assert!(
            !lower.contains("pnpm"),
            "must not hardcode pnpm, got: {prompt}"
        );
        assert!(
            !lower.contains("npm run"),
            "must not hardcode npm run, got: {prompt}"
        );
        assert!(
            !lower.contains("yarn"),
            "must not hardcode yarn, got: {prompt}"
        );
    }

    #[test]
    fn prompt_does_not_contain_push_or_pr_instructions() {
        let prompt = build_prompt(1, "title", "body", &[]);
        let lower = prompt.to_lowercase();
        assert!(
            !lower.contains("git push"),
            "must not instruct git push, got: {prompt}"
        );
        assert!(
            !lower.contains("open a pr"),
            "must not instruct opening a PR, got: {prompt}"
        );
        assert!(
            !lower.contains("create a pr"),
            "must not instruct creating a PR, got: {prompt}"
        );
        assert!(
            !lower.contains("gh pr create"),
            "must not instruct gh pr create, got: {prompt}"
        );
        assert!(
            !lower.contains("pull request"),
            "must not mention pull request, got: {prompt}"
        );
    }
}
