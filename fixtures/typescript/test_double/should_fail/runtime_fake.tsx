export class FakeUserRepository {
  get(id: string) {
    return <span>{id}</span>;
  }
}
