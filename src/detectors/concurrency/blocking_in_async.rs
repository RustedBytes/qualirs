use crate::analysis::{detector::Detector, evidence};
use crate::domain::{
    smell::{FindingConfidence, Severity, Smell, SmellCategory, SourceLocation},
    source::SourceFile,
};
use syn::spanned::Spanned;
pub struct BlockingInAsyncDetector;
impl Detector for BlockingInAsyncDetector {
    fn name(&self) -> &str {
        "Blocking in Async"
    }
    fn detect(&self, file: &SourceFile) -> Vec<Smell> {
        let mut findings = Vec::new();
        evidence::inspect(&file.ast, |expr, ctx| {
            if ctx.in_async
                && let syn::Expr::Call(call) = expr
                && let syn::Expr::Path(path) = &*call.func
            {
                let path = ctx.path(&path.path);
                if path.starts_with("std::") && evidence::is_fs_result(&path)
                    || matches!(
                        path.as_str(),
                        "std::thread::sleep"
                            | "std::thread::park"
                            | "std::thread::park_timeout"
                            | "std::net::TcpStream::connect"
                            | "std::net::TcpListener::bind"
                            | "std::net::UdpSocket::bind"
                    )
                {
                    findings.push((
                        expr.span().start().line,
                        format!("Blocking call in async execution: {path}"),
                    ));
                }
            }
        });
        findings
            .into_iter()
            .map(|(line, message)| {
                Smell::new(
                    SmellCategory::Concurrency,
                    "Blocking in Async",
                    Severity::Warning,
                    FindingConfidence::High,
                    SourceLocation::new(file.path.clone(), line, line, None),
                    message,
                    "Use an async alternative or execute blocking work with spawn_blocking.",
                )
            })
            .collect()
    }
}
