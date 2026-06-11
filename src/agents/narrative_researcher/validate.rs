use super::types::{
    CaptureClaimInput, CaptureNarrativeItemsInput, CaptureNarrativeSideInput,
    CaptureOrientationInput, CaptureResearchGapInput, CaptureSectionInput, CaptureSourceInput,
    CLAIM_CONFIDENCES, CLAIM_SIDES, CLAIM_TYPES, MIN_CLAIMS, MIN_CRUXES, MIN_NARRATIVE_BODY_LEN,
    MIN_SOURCES, NARRATIVE_ITEM_TYPES, NARRATIVE_SIDES, SECTION_KEYS, SOURCE_TYPES,
};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ValidationError {
    #[error("{0}")]
    Invalid(String),
}

impl ValidationError {
    fn invalid(message: impl Into<String>) -> Self {
        Self::Invalid(message.into())
    }
}

pub type Result<T = ()> = std::result::Result<T, ValidationError>;

pub fn validate_source(input: &CaptureSourceInput) -> Result {
    if input.title.trim().is_empty() {
        return Err(ValidationError::invalid("source title cannot be empty"));
    }
    if input.why_it_matters.trim().len() < 20 {
        return Err(ValidationError::invalid(
            "why_it_matters must be at least 20 characters",
        ));
    }
    if !SOURCE_TYPES.contains(&input.source_type.as_str()) {
        return Err(ValidationError::invalid(format!(
            "invalid source_type '{}'; expected one of: {}",
            input.source_type,
            SOURCE_TYPES.join(", ")
        )));
    }
    Ok(())
}

pub fn validate_claim(input: &CaptureClaimInput) -> Result {
    if input.claim.trim().is_empty() {
        return Err(ValidationError::invalid("claim cannot be empty"));
    }
    if input.confidence != "inference"
        && input.source_id.is_none()
        && input.source_title.as_deref().unwrap_or("").trim().is_empty()
    {
        return Err(ValidationError::invalid(
            "non-inference claims require source_id or source_title",
        ));
    }
    if !CLAIM_TYPES.contains(&input.claim_type.as_str()) {
        return Err(ValidationError::invalid(format!(
            "invalid claim_type '{}'",
            input.claim_type
        )));
    }
    if !CLAIM_SIDES.contains(&input.side.as_str()) {
        return Err(ValidationError::invalid(format!("invalid side '{}'", input.side)));
    }
    if !CLAIM_CONFIDENCES.contains(&input.confidence.as_str()) {
        return Err(ValidationError::invalid(format!(
            "invalid confidence '{}'",
            input.confidence
        )));
    }
    Ok(())
}

pub fn validate_claim_relaxed(input: &CaptureClaimInput) -> Result {
    if input.claim.trim().is_empty() {
        return Err(ValidationError::invalid("claim cannot be empty"));
    }
    Ok(())
}

pub fn validate_narrative_side(input: &CaptureNarrativeSideInput) -> Result {
    if !NARRATIVE_SIDES.contains(&input.side.as_str()) {
        return Err(ValidationError::invalid(format!(
            "invalid narrative side '{}'; expected one of: {}",
            input.side,
            NARRATIVE_SIDES.join(", ")
        )));
    }
    let min_len = if input.side == "counter_narrative" {
        20
    } else {
        MIN_NARRATIVE_BODY_LEN
    };
    if input.body.trim().len() < min_len {
        return Err(ValidationError::invalid(format!(
            "{} narrative body must be at least {min_len} characters",
            input.side
        )));
    }
    Ok(())
}

pub fn validate_narrative_items(input: &CaptureNarrativeItemsInput) -> Result {
    if !NARRATIVE_ITEM_TYPES.contains(&input.item_type.as_str()) {
        return Err(ValidationError::invalid(format!(
            "invalid item_type '{}'; expected agreement or crux",
            input.item_type
        )));
    }
    if input.items.is_empty() {
        return Err(ValidationError::invalid("items array cannot be empty"));
    }
    for item in &input.items {
        if item.trim().len() < 15 {
            return Err(ValidationError::invalid(
                "each narrative item must be at least 15 characters",
            ));
        }
    }
    Ok(())
}

pub fn validate_orientation(input: &CaptureOrientationInput) -> Result {
    for (field, value) in [
        ("dominant_question", &input.dominant_question),
        ("current_setup", &input.current_setup),
        ("time_horizon", &input.time_horizon),
    ] {
        if value.trim().is_empty() {
            return Err(ValidationError::invalid(format!("{field} cannot be empty")));
        }
    }
    Ok(())
}

pub fn validate_section(input: &CaptureSectionInput) -> Result {
    if !SECTION_KEYS.contains(&input.section_key.as_str()) {
        return Err(ValidationError::invalid(format!(
            "invalid section_key '{}'; expected business_model or why_now",
            input.section_key
        )));
    }
    if input.body.trim().len() < 40 {
        return Err(ValidationError::invalid(
            "section body must be at least 40 characters",
        ));
    }
    Ok(())
}

pub fn validate_research_gap(input: &CaptureResearchGapInput) -> Result {
    if input.gap_key.trim().is_empty() || input.description.trim().is_empty() {
        return Err(ValidationError::invalid(
            "gap_key and description are required",
        ));
    }
    Ok(())
}

/// Full workspace validation before finalize_narrative_research succeeds.
pub fn validate_workspace_ready(
    source_count: i64,
    claim_count: i64,
    dominant: Option<&str>,
    bull: Option<&str>,
    bear: Option<&str>,
    consensus: Option<&str>,
    crux_count: i64,
    orientation_captured: bool,
    business_model_captured: bool,
    why_now_captured: bool,
) -> Result {
    let mut errors = Vec::new();

    if source_count < MIN_SOURCES as i64 {
        errors.push(format!("need at least {MIN_SOURCES} sources, have {source_count}"));
    }
    if claim_count < MIN_CLAIMS as i64 {
        errors.push(format!("need at least {MIN_CLAIMS} claims, have {claim_count}"));
    }
    for (label, value) in [
        ("dominant", dominant),
        ("bull", bull),
        ("bear", bear),
        ("consensus", consensus),
    ] {
        match value.map(str::trim).filter(|text| !text.is_empty()) {
            Some(text) if text.len() >= MIN_NARRATIVE_BODY_LEN => {}
            Some(_) => errors.push(format!("{label} narrative is too short")),
            None => errors.push(format!("{label} narrative is missing")),
        }
    }
    if let (Some(bull_text), Some(bear_text)) = (bull, bear) {
        if bull_text.trim().eq_ignore_ascii_case(bear_text.trim()) {
            errors.push("bull and bear narratives must differ".to_string());
        }
    }
    if crux_count < MIN_CRUXES as i64 {
        errors.push(format!(
            "need at least {MIN_CRUXES} crux items, have {crux_count}"
        ));
    }
    if !orientation_captured {
        errors.push("orientation section is missing".to_string());
    }
    if !business_model_captured {
        errors.push("business_model section is missing".to_string());
    }
    if !why_now_captured {
        errors.push("why_now section is missing".to_string());
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(ValidationError::invalid(format!(
            "narrative research is incomplete:\n- {}",
            errors.join("\n- ")
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_short_bull_narrative() {
        let input = CaptureNarrativeSideInput {
            side: "bull".to_string(),
            body: "too short".to_string(),
        };
        assert!(validate_narrative_side(&input).is_err());
    }

    #[test]
    fn workspace_ready_requires_cruxes() {
        let long = "x".repeat(MIN_NARRATIVE_BODY_LEN);
        assert!(validate_workspace_ready(
            5,
            6,
            Some(&long),
            Some(&long),
            Some(&format!("{long} bear variant")),
            Some(&long),
            1,
            true,
            true,
            true,
        )
        .is_err());
    }
}
