export class Matrix {
    /**
     * Number of rows of the matrix.
     */
    readonly rows: number;

    /**
     * Number of columns of the matrix.
     */
    readonly columns: number;


    public get size(): number {
        return this.rows * this.columns;
    }

    constructor(private data: number[][]) {
        this.rows = data.length;
        this.columns = data[0].length;
    }

    get(rowIndex: number, columnIndex: number): number {
        return this.data[rowIndex][columnIndex];
    }

    set(rowIndex: number, columnIndex: number, value: number): void {
        this.data[rowIndex][columnIndex] = value;
    }

    static rowVector(newData: number[]): Matrix {
        let data = new Array(1)
        data[0] = new Array(newData.length)
        for (let i = 0; i < newData.length; i++) {
            data[0][i] = newData[i];
        }        
        return new Matrix(data);
    }

    static columnVector(newData: number[]): Matrix {
        let data = new Array(newData.length);
        for (let i = 0; i < newData.length; i++) {
            data[i] = new Array(1);
            data[i][0] = newData[i];
        }
        return new Matrix(data);
    }

    static zeros(rows: number, columns: number): Matrix {
        let data = new Array(rows);
        for (let i = 0; i < rows; i++) {
            data[i] = new Array(columns);
            data[i].fill(0);
        }
        return new Matrix(data);
    }

    static ones(rows: number, columns: number): Matrix {
        let data = new Array(rows);
        for (let i = 0; i < rows; i++) {
            data[i] = new Array(columns)
            data[i].fill(1);
        }
        return new Matrix(data);
    }

    static identity(rows: number, columns?: number, value?: number): Matrix {
        if (columns === undefined) columns = rows;
        if (value === undefined) value = 1;
        let min = Math.min(rows, columns);
        let matrix = this.zeros(rows, columns);
        for (let i = 0; i < min; i++) {
            matrix.set(i, i, value);
        }
        return matrix;
    }

    static diag(data: number[], rows?: number, columns?: number): Matrix {
        let l = data.length;
        if (rows === undefined) rows = l;
        if (columns === undefined) columns = rows;
        let min = Math.min(l, rows, columns);
        let matrix = this.zeros(rows, columns);
        for (let i = 0; i < min; i++) {
            matrix.set(i, i, data[i]);
        }
        return matrix;
    }

    /**
    * Returns a matrix whose elements are the minimum between `matrix1` and `matrix2`.
    */
    static min(matrix1: Matrix, matrix2: Matrix): Matrix {
        let rows = matrix1.rows;
        let columns = matrix1.columns;
        let result = this.zeros(rows, columns);
        for (let i = 0; i < rows; i++) {
            for (let j = 0; j < columns; j++) {
                result.set(i, j, Math.min(matrix1.get(i, j), matrix2.get(i, j)));
            }
        }
        return result;
    }

    /**
     * Returns a matrix whose elements are the maximum between `matrix1` and `matrix2`.
     * @param matrix1
     * @param matrix2
     */
    static max(matrix1: Matrix, matrix2: Matrix): Matrix {
        let rows = matrix1.rows;
        let columns = matrix1.columns;
        let result = this.zeros(rows, columns);
        for (let i = 0; i < rows; i++) {
            for (let j = 0; j < columns; j++) {
                result.set(i, j, Math.max(matrix1.get(i, j), matrix2.get(i, j)));
            }
        }
        return result;
    }

    to1DArray(): number[] {
        let array = new Array(this.size);
        for (let i = 0; i < this.rows; i++) {
            for (let j = 0; j < this.columns; j++) {
                array[i * this.columns + j] = this.get(i, j);
            }
        }
        return array;
    }

    /**
     * Computes the dot (scalar) product between the matrix and another.
     * @param vector
     */
    dot(vector: Matrix): number {
        let vector1 = this.to1DArray();
        let vector2 = vector.to1DArray();
        if (vector1.length !== vector2.length) {
            throw new RangeError('vectors do not have the same size');
        }
        let dot = 0;
        for (let i = 0; i < vector1.length; i++) {
            dot += vector1[i] * vector2[i];
        }
        return dot;
    }

    /**
     * Returns the matrix product between `this` and `other`.
     * @param other - Other matrix.
     */
    mmul(other: Matrix): Matrix {
        let m = this.rows;
        let n = this.columns;
        let p = other.columns;

        let result = Matrix.zeros(m, p);

        let Bcolj = new Array(n);
        for (let j = 0; j < p; j++) {
            for (let k = 0; k < n; k++) {
                Bcolj[k] = other.get(k, j);
            }

            for (let i = 0; i < m; i++) {
                let s = 0;
                for (let k = 0; k < n; k++) {
                    s += this.get(i, k) * Bcolj[k];
                }

                result.set(i, j, s);
            }
        }
        return result;
    }

    /**
     * Transposes the matrix and returns a new one containing the result.
     */
    transpose(): Matrix {
        let result = Matrix.zeros(this.columns, this.rows);
        for (let i = 0; i < this.rows; i++) {
            for (let j = 0; j < this.columns; j++) {
                result.set(j, i, this.get(i, j));
            }
        }
        return result;
    }

    /**
     * Returns the trace of the matrix (sum of the diagonal elements).
     */
    trace(): number {
        let min = Math.min(this.rows, this.columns);
        let trace = 0;
        for (let i = 0; i < min; i++) {
            trace += this.get(i, i);
        }
        return trace;
    }

    /**
     * Returns whether the number of rows or columns (or both) is zero.
     */
    isEmpty(): boolean {
        return this.rows === 0 || this.columns === 0;
    }

    clone(): Matrix {
        let result = Matrix.zeros(this.rows, this.columns);
        for (let i = 0; i < this.rows; i++) {
            for (let j = 0; j < this.columns; j++) {
                let value = this.get(i, j);
                result.set(i, j, value);
            }
        }
        return result;
    }

    isSquare(): boolean {
        return this.rows === this.columns;
    }

    isSymmetric(): boolean {
        if (!this.isSquare()) {
            return false;
        }
        for (let i = 0; i < this.rows; i++) {
            for (let j = 0; j < this.columns; j++) {
                if (this.get(i, j) !== this.get(j, i)) {
                    return false;
                }
            }
        }
        return true;
    }
}
