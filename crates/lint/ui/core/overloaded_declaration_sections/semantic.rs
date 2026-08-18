#![allow(dead_code, rlib::misordered_module_declarations)]
#![warn(rlib::overloaded_declaration_sections)]

// -----------------------------------------------------------------------------
// Accepted: Boundary-sized family
// -----------------------------------------------------------------------------

struct Accepted;

struct AcceptedBuilder;

struct AcceptedConfig;

struct AcceptedError;

struct AcceptedInput;

struct AcceptedExtra06;

struct AcceptedExtra07;

struct AcceptedExtra08;

struct AcceptedExtra09;

struct AcceptedExtra10;

struct AcceptedExtra11;

struct AcceptedExtra12;

struct AcceptedExtra13;

struct AcceptedExtra14;

struct AcceptedExtra15;

struct AcceptedExtra16;

struct AcceptedExtra17;

struct AcceptedExtra18;

struct AcceptedExtra19;

struct AcceptedExtra20;

struct AcceptedExtra21;

struct AcceptedExtra22;

struct AcceptedExtra23;

struct AcceptedExtra24;

struct AcceptedExtra25;

struct AcceptedExtra26;

struct AcceptedExtra27;

struct AcceptedExtra28;

struct AcceptedExtra29;

struct AcceptedExtra30;

struct AcceptedExtra31;

struct AcceptedExtra32;

struct AcceptedExtra33;

struct AcceptedExtra34;

struct AcceptedExtra35;

struct AcceptedExtra36;

struct AcceptedExtra37;

struct AcceptedExtra38;

struct AcceptedExtra39;

struct AcceptedExtra40;

// -----------------------------------------------------------------------------
// Transport: Mixed transport concepts
// -----------------------------------------------------------------------------

struct Request;

struct RequestBuilder;

struct RequestHeaders;

struct Response;

struct ResponseBuilder;

struct TransportError;

struct TransportBridge06;

struct TransportBridge07;

struct TransportBridge08;

struct TransportBridge09;

struct TransportBridge10;

struct TransportBridge11;

struct TransportBridge12;

struct TransportBridge13;

struct TransportBridge14;

struct TransportBridge15;

struct TransportBridge16;

struct TransportBridge17;

struct TransportBridge18;

struct TransportBridge19;

struct TransportBridge20;

struct TransportBridge21;

struct TransportBridge22;

struct TransportBridge23;

struct TransportBridge24;

struct TransportBridge25;

struct TransportBridge26;

struct TransportBridge27;

struct TransportBridge28;

struct TransportBridge29;

struct TransportBridge30;

struct TransportBridge31;

struct TransportBridge32;

struct TransportBridge33;

struct TransportBridge34;

struct TransportBridge35;

struct TransportBridge36;

struct TransportBridge37;

struct TransportBridge38;

struct TransportBridge39;

struct TransportBridge40;

// -----------------------------------------------------------------------------
// Cohesive: Single naming family
// -----------------------------------------------------------------------------

struct Cohesive;

struct CohesiveBuilder;

struct CohesiveConfig;

struct CohesiveError;

struct CohesiveInput;

struct CohesiveOutput;

struct CohesiveExtra06;

struct CohesiveExtra07;

struct CohesiveExtra08;

struct CohesiveExtra09;

struct CohesiveExtra10;

struct CohesiveExtra11;

struct CohesiveExtra12;

struct CohesiveExtra13;

struct CohesiveExtra14;

struct CohesiveExtra15;

struct CohesiveExtra16;

struct CohesiveExtra17;

struct CohesiveExtra18;

struct CohesiveExtra19;

struct CohesiveExtra20;

struct CohesiveExtra21;

struct CohesiveExtra22;

struct CohesiveExtra23;

struct CohesiveExtra24;

struct CohesiveExtra25;

struct CohesiveExtra26;

struct CohesiveExtra27;

struct CohesiveExtra28;

struct CohesiveExtra29;

struct CohesiveExtra30;

struct CohesiveExtra31;

struct CohesiveExtra32;

struct CohesiveExtra33;

struct CohesiveExtra34;

struct CohesiveExtra35;

struct CohesiveExtra36;

struct CohesiveExtra37;

struct CohesiveExtra38;

struct CohesiveExtra39;

struct CohesiveExtra40;

// -----------------------------------------------------------------------------
// NominalMixed: A nominal type must not hide unrelated values
// -----------------------------------------------------------------------------

struct NominalMixed;

fn billing_total() {}

fn cache_limit() {}

fn delivery_window() {}

fn parser_mode() {}

fn retry_count() {}

fn audit_metric_06() {}

fn audit_metric_07() {}

fn audit_metric_08() {}

fn audit_metric_09() {}

fn audit_metric_10() {}

fn audit_metric_11() {}

fn audit_metric_12() {}

fn audit_metric_13() {}

fn audit_metric_14() {}

fn audit_metric_15() {}

fn audit_metric_16() {}

fn audit_metric_17() {}

fn audit_metric_18() {}

fn audit_metric_19() {}

fn audit_metric_20() {}

fn audit_metric_21() {}

fn audit_metric_22() {}

fn audit_metric_23() {}

fn audit_metric_24() {}

fn audit_metric_25() {}

fn audit_metric_26() {}

fn audit_metric_27() {}

fn audit_metric_28() {}

fn audit_metric_29() {}

fn audit_metric_30() {}

fn audit_metric_31() {}

fn audit_metric_32() {}

fn audit_metric_33() {}

fn audit_metric_34() {}

fn audit_metric_35() {}

fn audit_metric_36() {}

fn audit_metric_37() {}

fn audit_metric_38() {}

fn audit_metric_39() {}

fn audit_metric_40() {}

mod module_namespace {
    // -----------------------------------------------------------------------------
    // ModuleNamespace: Module-owned operations
    // -----------------------------------------------------------------------------

    fn first() {}

    fn second() {}

    fn third() {}

    fn fourth() {}

    fn fifth() {}

    fn sixth() {}

    fn module_task_07() {}

    fn module_task_08() {}

    fn module_task_09() {}

    fn module_task_10() {}

    fn module_task_11() {}

    fn module_task_12() {}

    fn module_task_13() {}

    fn module_task_14() {}

    fn module_task_15() {}

    fn module_task_16() {}

    fn module_task_17() {}

    fn module_task_18() {}

    fn module_task_19() {}

    fn module_task_20() {}

    fn module_task_21() {}

    fn module_task_22() {}

    fn module_task_23() {}

    fn module_task_24() {}

    fn module_task_25() {}

    fn module_task_26() {}

    fn module_task_27() {}

    fn module_task_28() {}

    fn module_task_29() {}

    fn module_task_30() {}

    fn module_task_31() {}

    fn module_task_32() {}

    fn module_task_33() {}

    fn module_task_34() {}

    fn module_task_35() {}

    fn module_task_36() {}

    fn module_task_37() {}

    fn module_task_38() {}

    fn module_task_39() {}

    fn module_task_40() {}

    fn module_task_41() {}
}

// -----------------------------------------------------------------------------
// Collapsed: One declaration with several implementations
// -----------------------------------------------------------------------------

struct Collapsed;

impl Collapsed {
    fn first(&self) {}
}

impl Collapsed {
    fn second(&self) {}
}

fn main() {}
