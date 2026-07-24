# Production Deployment Runbook

## Terraform Remote State

Both `terraform/` and `infra/terraform/` use an S3 backend with DynamoDB
state locking (`encrypt = true`). This prevents two engineers from running
Terraform against the same module at the same time and corrupting the state
file or creating conflicting infrastructure.

The bucket and lock table names are not hardcoded in the `.tf` files —
they're supplied at `terraform init` time (partial backend configuration)
from two GitHub Actions secrets:

- `TERRAFORM_STATE_BUCKET` — the S3 bucket holding state files
- `TERRAFORM_LOCK_TABLE` — the DynamoDB table used for state locking

### One-time setup (per AWS account)

```bash
aws s3api create-bucket --bucket "$TERRAFORM_STATE_BUCKET" --region af-south-1 \
  --create-bucket-configuration LocationConstraint=af-south-1
aws s3api put-bucket-versioning --bucket "$TERRAFORM_STATE_BUCKET" \
  --versioning-configuration Status=Enabled
aws s3api put-bucket-encryption --bucket "$TERRAFORM_STATE_BUCKET" \
  --server-side-encryption-configuration '{"Rules":[{"ApplyServerSideEncryptionByDefault":{"SSEAlgorithm":"AES256"}}]}'

aws dynamodb create-table --table-name "$TERRAFORM_LOCK_TABLE" \
  --attribute-definitions AttributeName=LockID,AttributeType=S \
  --key-schema AttributeName=LockID,KeyType=HASH \
  --billing-mode PAY_PER_REQUEST
```

### Initializing a module

Run from the module directory (`terraform/` or `infra/terraform/`):

```bash
terraform init \
  -backend-config="bucket=$TERRAFORM_STATE_BUCKET" \
  -backend-config="dynamodb_table=$TERRAFORM_LOCK_TABLE"
```

In CI, export both values from the corresponding repository secrets before
calling `terraform init`.

### Applying changes

```bash
terraform plan -out=tfplan
terraform apply tfplan
```

If another engineer (or CI run) holds the state lock, `apply`/`plan` will
block with a `ConditionalCheckFailedException` until it's released — do not
force-unlock (`terraform force-unlock`) unless you've confirmed the other
run is actually dead.
