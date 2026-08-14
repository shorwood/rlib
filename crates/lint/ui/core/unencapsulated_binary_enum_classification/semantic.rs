#![allow(dead_code, misordered_module_declarations)]
#![warn(unencapsulated_binary_enum_classification)]

enum FunctionReturn {
    Unit,
    Value,
}

impl FunctionReturn {
    fn from_returns_unit(returns_unit: bool) -> Self {
        if returns_unit {
            Self::Unit
        } else {
            Self::Value
        }
    }
}

fn classify_externally(returns_unit: bool) -> FunctionReturn {
    if returns_unit {
        FunctionReturn::Unit
    } else {
        FunctionReturn::Value
    }
}

fn classify_with_explicit_returns(returns_unit: bool) -> FunctionReturn {
    if returns_unit {
        return FunctionReturn::Unit;
    } else {
        return FunctionReturn::Value;
    }
}

fn classify_with_reversed_polarity(returns_value: bool) -> FunctionReturn {
    if returns_value {
        FunctionReturn::Value
    } else {
        FunctionReturn::Unit
    }
}

trait ClassifyExternally {
    fn classify(returns_unit: bool) -> Self;
}

impl ClassifyExternally for FunctionReturn {
    fn classify(returns_unit: bool) -> Self {
        if returns_unit {
            Self::Unit
        } else {
            Self::Value
        }
    }
}

#[repr(u8)]
enum DiscriminatedBinary {
    Disabled = 0,
    Enabled = 1,
}

fn classify_discriminated(enabled: bool) -> DiscriminatedBinary {
    if enabled {
        DiscriminatedBinary::Enabled
    } else {
        DiscriminatedBinary::Disabled
    }
}

enum TernaryState {
    Ready,
    Waiting,
    Failed,
}

fn classify_non_binary(is_ready: bool) -> TernaryState {
    if is_ready {
        TernaryState::Ready
    } else {
        TernaryState::Waiting
    }
}

enum PayloadState {
    Empty,
    Loaded(u32),
}

fn classify_payload(has_value: bool) -> PayloadState {
    if has_value {
        PayloadState::Loaded(1)
    } else {
        PayloadState::Empty
    }
}

fn incomplete_mapping(is_unit: bool) -> FunctionReturn {
    if is_unit {
        FunctionReturn::Unit
    } else {
        FunctionReturn::Unit
    }
}

macro_rules! generated_classification {
    ($is_unit:expr) => {
        if $is_unit {
            FunctionReturn::Unit
        } else {
            FunctionReturn::Value
        }
    };
}

fn generated_mapping(is_unit: bool) -> FunctionReturn {
    generated_classification!(is_unit)
}

#[allow(unencapsulated_binary_enum_classification)]
fn deliberately_external(is_unit: bool) -> FunctionReturn {
    if is_unit {
        FunctionReturn::Unit
    } else {
        FunctionReturn::Value
    }
}

fn main() {}
