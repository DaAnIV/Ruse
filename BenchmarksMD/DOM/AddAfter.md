### DOM

```html
<document>
  <body>
    <p id="ref">
    </p>
  </body>
</document>
```

```html
<document>
  <body>
    <p id="ref">
    </p>
    <p id="new">
    </p>
  </body>
</document>
```

### Sketch

```typescript
add_after(node_id: string, new_node: HTMLElement): void {
    ???
};
```

### Solution

```typescript
add_after(node_id: string, new_node: HTMLElement): void {
  let node = document.getElementById(node_id);
  node.parentNode.insertBefore(new_node, node.nextSibling);
};
```

Or As of 2021

```typescript
add_after(node_id: string, new_node: HTMLElement): void {
  let node = document.getElementById(node_id);
  node.after(new_node);
};
```
