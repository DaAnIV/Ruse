package linear;

public class Matrix {
    /**
     * Number of rows of the matrix.
     */
    public final int rows;

    /**
     * Number of columns of the matrix.
     */
    public final int columns;

    private final double[][] data;

    public Matrix(double[][] data) {
        this.data = new double[data.length][];
        for (int i = 0; i < data.length; i++) {
            this.data[i] = data[i].clone();
        }
        this.rows = data.length;
        this.columns = data[0].length;
    }

    public int size() {
        return this.rows * this.columns;
    }

    public double get(int rowIndex, int columnIndex) {
        return this.data[rowIndex][columnIndex];
    }

    public void set(int rowIndex, int columnIndex, double value) {
        this.data[rowIndex][columnIndex] = value;
    }

    public static Matrix rowVector(double[] newData) {
        double[][] data = new double[1][newData.length];
        System.arraycopy(newData, 0, data[0], 0, newData.length);
        return new Matrix(data);
    }

    public static Matrix columnVector(double[] newData) {
        double[][] data = new double[newData.length][1];
        for (int i = 0; i < newData.length; i++) {
            data[i][0] = newData[i];
        }
        return new Matrix(data);
    }

    public static Matrix zeros(int rows, int columns) {
        double[][] data = new double[rows][columns];
        return new Matrix(data);
    }

    public static Matrix ones(int rows, int columns) {
        double[][] data = new double[rows][columns];
        for (int i = 0; i < rows; i++) {
            for (int j = 0; j < columns; j++) {
                data[i][j] = 1.0;
            }
        }
        return new Matrix(data);
    }

    public static Matrix identity(int rows) {
        return identity(rows, rows, 1.0);
    }

    public static Matrix identity(int rows, int columns) {
        return identity(rows, columns, 1.0);
    }

    public static Matrix identity(int rows, int columns, double value) {
        int min = Math.min(rows, columns);
        Matrix matrix = zeros(rows, columns);
        for (int i = 0; i < min; i++) {
            matrix.set(i, i, value);
        }
        return matrix;
    }

    public static Matrix diag(double[] data) {
        return diag(data, data.length, data.length);
    }

    public static Matrix diag(double[] data, int rows) {
        return diag(data, rows, rows);
    }

    public static Matrix diag(double[] data, int rows, int columns) {
        int l = data.length;
        int min = Math.min(l, Math.min(rows, columns));
        Matrix matrix = zeros(rows, columns);
        for (int i = 0; i < min; i++) {
            matrix.set(i, i, data[i]);
        }
        return matrix;
    }

    /**
     * Returns a matrix whose elements are the minimum between matrix1 and matrix2.
     */
    public static Matrix min(Matrix matrix1, Matrix matrix2) {
        int rows = matrix1.rows;
        int columns = matrix1.columns;
        Matrix result = zeros(rows, columns);
        for (int i = 0; i < rows; i++) {
            for (int j = 0; j < columns; j++) {
                result.set(i, j, Math.min(matrix1.get(i, j), matrix2.get(i, j)));
            }
        }
        return result;
    }

    /**
     * Returns a matrix whose elements are the maximum between matrix1 and matrix2.
     */
    public static Matrix max(Matrix matrix1, Matrix matrix2) {
        int rows = matrix1.rows;
        int columns = matrix1.columns;
        Matrix result = zeros(rows, columns);
        for (int i = 0; i < rows; i++) {
            for (int j = 0; j < columns; j++) {
                result.set(i, j, Math.max(matrix1.get(i, j), matrix2.get(i, j)));
            }
        }
        return result;
    }

    public double[] to1DArray() {
        double[] array = new double[size()];
        for (int i = 0; i < this.rows; i++) {
            for (int j = 0; j < this.columns; j++) {
                array[i * this.columns + j] = this.get(i, j);
            }
        }
        return array;
    }

    /**
     * Computes the dot (scalar) product between the matrix and another.
     */
    public double dot(Matrix vector) {
        double[] vector1 = this.to1DArray();
        double[] vector2 = vector.to1DArray();
        if (vector1.length != vector2.length) {
            throw new IllegalArgumentException("vectors do not have the same size");
        }
        double dot = 0;
        for (int i = 0; i < vector1.length; i++) {
            dot += vector1[i] * vector2[i];
        }
        return dot;
    }

    /**
     * Returns the matrix product between this and other.
     */
    public Matrix mmul(Matrix other) {
        int m = this.rows;
        int n = this.columns;
        int p = other.columns;

        Matrix result = zeros(m, p);

        double[] Bcolj = new double[n];
        for (int j = 0; j < p; j++) {
            for (int k = 0; k < n; k++) {
                Bcolj[k] = other.get(k, j);
            }

            for (int i = 0; i < m; i++) {
                double s = 0;
                for (int k = 0; k < n; k++) {
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
    public Matrix transpose() {
        Matrix result = zeros(this.columns, this.rows);
        for (int i = 0; i < this.rows; i++) {
            for (int j = 0; j < this.columns; j++) {
                result.set(j, i, this.get(i, j));
            }
        }
        return result;
    }

    /**
     * Returns the trace of the matrix (sum of the diagonal elements).
     */
    public double trace() {
        int min = Math.min(this.rows, this.columns);
        double trace = 0;
        for (int i = 0; i < min; i++) {
            trace += this.get(i, i);
        }
        return trace;
    }

    /**
     * Returns whether the number of rows or columns (or both) is zero.
     */
    public boolean isEmpty() {
        return this.rows == 0 || this.columns == 0;
    }

    public Matrix clone() {
        Matrix result = zeros(this.rows, this.columns);
        for (int i = 0; i < this.rows; i++) {
            for (int j = 0; j < this.columns; j++) {
                double value = this.get(i, j);
                result.set(i, j, value);
            }
        }
        return result;
    }

    public boolean isSquare() {
        return this.rows == this.columns;
    }

    public boolean isSymmetric() {
        if (!this.isSquare()) {
            return false;
        }
        for (int i = 0; i < this.rows; i++) {
            for (int j = 0; j < this.columns; j++) {
                if (this.get(i, j) != this.get(j, i)) {
                    return false;
                }
            }
        }
        return true;
    }
}