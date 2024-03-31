# User Full Name

## Sketch

```typescript
export class Packet {
    constructor(public headers: Record<string, string>) {}

    public is_loopback(): bool {
        ???
    }
}
```

## Input - Output

input:
[
{
`this` $\mapsto$ `{"headers": {"smac": "a", "dmac": "b"}}`
},
{
`this` $\mapsto$ `{"headers": {"smac": "a", "dmac": "a"}}`
}
]

output:
[
`false`, `true`
]

## Solution

```typescript
export class Packet {
    constructor(public headers: Record<string, string>) {}

    public is_loopback(): bool {
        return this.headers.smac === this.headers.dmac;
    }
}
```
