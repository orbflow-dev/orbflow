fn main() {
    let _x: Result<(), &'static str> = (|| {
        let _ = std::fs::canonicalize("nonexistent").map_err(|_| "canonicalize_failed")?;
        Ok(())
    })();
}
