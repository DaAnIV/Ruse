# Binary Search tree delete

## Sketch

```typescript
export class BinarySearchTree<T> {
    root?: TreeNode<T>;
    comparator: (a: T, b: T) => number;
    ...
    public length(): number {...}
    public contains(val: T): bool {...}

    public remove(val: T): void {
        let node = this.find_node(val);
        if (!node) { return; }

        if (!node.right && !node.left) {
            this.unlink_leaf(node);
        } else if (!node.right || !node.left) {
            ...
        } else {
            ??? // Synthesizer help
        }
    }

    unlink_leaf(node: TreeNode<T>): void {...}
    find_node(val: T): TreeNode<T> | null {...}
}

valid_binary_search_tree(root: TreeNode<T>): bool {...}
replace(a: TreeNode<T>, b: TreeNode<T>): void {...}
minimum(root: TreeNode<T>): TreeNode<T> {...}
maximum(root: TreeNode<T>): TreeNode<T> {...}
```

## Input - Output

input:
`T` $\mapsto$ `number`
`this.root` $\mapsto$ `create_tree([5, [[2, [1,3]], [7, [6, 8]]])`
`this.comparator` $\mapsto$ `(a: number, b: number) => { return a - b; }`
`val` $\mapsto$ 7

output:

```typescript
!this.contains(7) && valid_binary_search_tree(this.root) && this.length() === 6
```

## Solution

```typescript
public remove(val: T): void {
    let node = this.find_node(val);
    if (!node) { return; }

    if (!node.right && !node.left) {
        this.unlink_leaf(node);
    } else if (!node.right || !node.left) {
        ...
    } else {
        let succ = minimum(node.right);
        replace(node, succ)
        this.unlink_leaf(succ);
    }
}
```
