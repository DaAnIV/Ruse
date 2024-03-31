### DOM

```html
<document>
  <body>
    <p id="foo">
        <span>hello</span>
        <div>world</div>
    </p>
  </body>
</document>
```

```html
<document>
  <body>
    <p id="foo">
    </p>
  </body>
</document>
```

### Sketch

```typescript
remove_all_children(node_id: string): void {
    ???
};
```

### Solution

```typescript
remove_all_children(node_id: string): void {
    // Needs to reparse DOM after this...
    document.getElementById(node_id).textContent = '';
};
```
