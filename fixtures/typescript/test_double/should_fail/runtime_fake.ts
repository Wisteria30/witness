export class FakeUserRepository {
  get(id: string): { id: string } {
    return { id }
  }
}
