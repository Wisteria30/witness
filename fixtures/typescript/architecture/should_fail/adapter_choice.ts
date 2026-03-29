import { SqlUserRepository } from "../infra/sql-user-repository"

export function buildService() {
  return new SqlUserRepository()
}
