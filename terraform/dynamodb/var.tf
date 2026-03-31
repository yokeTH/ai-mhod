variable "env_suffix" {
  description = "Suffix for table names (e.g., 'dev', 'prod', 'stagging')"
  type        = string
}

variable "tags" {
  type    = map(string)
  default = {}
}

variable "project_prefix" {
  description = "Prefix for table names (e.g., 'personal')"
  type        = string
}
