import { User } from "./user";

export class UserTuple {
    private constructor(private readonly values: User[]) {}
}
