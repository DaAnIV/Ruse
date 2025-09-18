package ruse;

public class BinaryTree {
    private int height;
    private int size;
    private double value;
    private BinaryTree left;
    private BinaryTree right;

    public BinaryTree(double value) {
        this(value, null, null);
    }

    public BinaryTree(double value, BinaryTree left, BinaryTree right) {
        this.value = value;
        this.left = left;
        this.right = right;
        this.size = 1;
        this.height = 1;
        this.resetHeightAndSize();
    }

    private void resetHeightAndSize() {
        this.size = 1;
        this.height = 1;
        
        if (this.right != null) {
            this.size += this.right.size;
            this.height = this.right.height + 1;
        }
        if (this.left != null) {
            this.size += this.left.size;
            if (this.left.height + 1 > this.height) {
                this.height = this.left.height + 1;
            }
        }
    }

    public BinaryTree getRight() {
        return this.right;
    }

    public void setRight(BinaryTree value) {
        this.right = value;
        this.resetHeightAndSize();
    }

    public BinaryTree getLeft() {
        return this.left;
    }

    public void setLeft(BinaryTree value) {
        this.left = value;
        this.resetHeightAndSize();
    }

    public int getSize() {
        return this.size;
    }

    public int getHeight() {
        return this.height;
    }

    public double getValue() {
        return this.value;
    }

    public void incValue() {
        this.value++;
    }
}