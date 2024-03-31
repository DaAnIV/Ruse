## Idea:
Input output examples are in HTML and out HTML (+ context)

### DOM

```html
<document>
  <body>
    <ul id="students">
    </ul>
  </body>
</document>
```

```html
<document>
  <body>
    <ul id="students">
        <li>John</li>
    </ul>
  </body>
</document>
```

### Sketch

```typescript
add_student(student: Student): void {
    ???
}
```

### Solution

```typescript
add_student(student: Student): void {
    let students = document.getElementById("students");
    let li = document.createElement("li");
    li.textContent = student.name;
    students.appendChild(li);
}
```
