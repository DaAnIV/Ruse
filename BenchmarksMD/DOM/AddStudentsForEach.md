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
add_students(students: Student[]): void {
    ???
}
```

### Solution

```typescript
add_student(students: Student[]): void {
    let students = document.getElementById("students");
    students.forEach(x => {
      let li = document.createElement("li");
      li.textContent = x.name;
      students.appendChild(li);
    });
}
```
