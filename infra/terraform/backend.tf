# Remote state backend for infra/terraform/ (global_lb, edge, dr_bcp,
# horizontal_scaling). Prevents multiple engineers from running Terraform
# concurrently against this module and corrupting or overwriting state.
#
# `bucket` and `dynamodb_table` are supplied at `terraform init` time via
# `-backend-config`, sourced from the TERRAFORM_STATE_BUCKET and
# TERRAFORM_LOCK_TABLE GitHub Actions secrets — see
# docs/deployment/PRODUCTION_DEPLOYMENT_RUNBOOK.md.

terraform {
  backend "s3" {
    key     = "aframp-backend/infra/terraform.tfstate"
    region  = "af-south-1"
    encrypt = true
  }
}
