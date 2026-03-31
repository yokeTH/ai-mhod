module "ai_mhod_table" {
  source     = "../../dynamodb"
  env_suffix = "prod"
  project_prefix = "personal"

  tags = {
    Environment = "production"
  }
}
