export class BinaryTree {
    private _height: number
    private _size: number

    constructor(value: number);
    constructor(value: number, left?: BinaryTree, right?: BinaryTree);
    constructor(private _value: number, private _left?: BinaryTree | undefined, private _right?: BinaryTree | undefined) {
        this._size = 1;
        this._height = 1;
        this._reset_height_and_size();
    }

    private _reset_height_and_size(): void {
        this._size = 1;
        this._height = 1;
        
        if (this._right != null) {
            this._size += this._right.size;
            this._height = this._right.height + 1;
        }
        if (this._left != null) {
            this._size += this._left.size;
            if (this._left.height + 1 > this._height) {
                this._height = this._left.height + 1;
            }
        }
    }
     
    public get right(): BinaryTree | undefined {
        return this._right;
    }

    public set right(value: BinaryTree | undefined) {
        this._right = value;
        this._reset_height_and_size();
    }

    public get left(): BinaryTree | undefined {
        return this._left;
    }

    public set left(value: BinaryTree | undefined) {
        this._left = value;
        this._reset_height_and_size();
    }

    public get size(): number {
        return this._size;
    }

    public get height(): number {
        return this._height;
    }

    public get value(): number {
        return this._value;
    }

    public inc_value(): void {
        this._value++;
    }
}
