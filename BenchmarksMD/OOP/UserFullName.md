# User Full Name

## Sketch

```typescript
export class User {
    constructor(public name: string, 
                public surname: string) {}

    public get_full_name(): string {
        ??? 
    }
}
```

## Input - Output

input:
[
`this` $\mapsto$ `{"name": "John", "surname": "Doe"}`,
`this` $\mapsto$ `{"name": "Paul", "surname": "Simon"}`
]

output:

[
`"John Doe"`,
`"Paul Simon"`
]

## Solution

```typescript
export class User {
    constructor(public name: string, 
                public surname: string) {}

    public get_full_name() {
        return this.name + ' ' + this.surname;
    }
}
```
