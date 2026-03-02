// The outcome of a single expectation check
#[derive(Debug, Clone)]
pub enum AuditStatus {
    Pass,
    Fail { reason: String, actual_value: String },
    Missing { reason: String }, // Hardware wasn't found at all
}

// The complete record of a test case
#[derive(Debug, Clone)]
pub struct ValidationResult {
    pub subsystem: String,      // e.g., "USB" or "I2C"
    pub item_name: String,      // e.g., "Mule CAN Adapter"
    pub location: String,       // e.g., "Bus 3, Port 3-1.4"
    pub status: AuditStatus,
}