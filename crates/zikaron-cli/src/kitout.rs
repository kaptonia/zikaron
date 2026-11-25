use std::path::Path;

pub use zikaron_glue::pack::Bundle;
pub use zikaron_glue::pack::Trouble;
pub use zikaron_glue::pack::Landed;

pub use zikaron_glue::pack::Form;

pub fn export(out: &Path, b: Bundle, form: Form) -> Result<Landed, Trouble> {
    crate::seam();
    zikaron_glue::pack::export_as(out, b, form)
}
