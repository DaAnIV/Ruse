# User Full Name

## Sketch

```typescript
export class ExamStats {
    constructor(public students: string[], 
                public grades: number[]) {}

    public add(other: ExamStats): ExamStats {
        ???
    }
}
```

## Input - Output

input:
[
{
`this` $\mapsto$ `{"students": ["1", "2"], "grades": [70, 90]}`
`other` $\mapsto$ `{"students": ["3", "4"], "grades": [80, 100]}`
},
{
`this` $\mapsto$ `{"students": ["1", "2"], "grades": [70, 90]}`
`other` $\mapsto$ `{"students": ["5"], "grades": [30]}`
}
]

output:
[
`{"students": ["1", "2", "3", "4"], "grades": [70, 90, 80, 100]}`
`{"students": ["1", "2", "5"], "grades": [70, 90, 30]}`
]

## Solution

```typescript
export class ExamStats {
    constructor(public students: string[], 
                public grades: number[]) {}

    public add(other: ExamStats): ExamStats {
        return new ExamStats(
            this.students.concat(other.students),
            this.grades.concat(other.grades),
        )
    }
}
```
