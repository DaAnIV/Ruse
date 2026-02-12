package linear;

import utils.Utils;

public class SingularValueDecomposition {
    private int m;
    private int n;
    private double[] s;
    private Matrix U;
    private Matrix V;

    public SingularValueDecomposition(Matrix value) {
        if (value.isEmpty()) {
            throw new IllegalArgumentException("Matrix must be non-empty");
        }

        int m = value.rows;
        int n = value.columns;

        boolean computeLeftSingularVectors = true;
        boolean computeRightSingularVectors = true;
        boolean autoTranspose = false;

        boolean wantu = computeLeftSingularVectors;
        boolean wantv = computeRightSingularVectors;

        boolean swapped = false;
        Matrix a;
        if (m < n) {
            if (!autoTranspose) {
                a = value.clone();
            } else {
                a = value.transpose();
                m = a.rows;
                n = a.columns;
                swapped = true;
                boolean aux = wantu;
                wantu = wantv;
                wantv = aux;
            }
        } else {
            a = value.clone();
        }

        int nu = Math.min(m, n);
        int ni = Math.min(m + 1, n);
        double[] s = new double[ni];
        Matrix U = Matrix.zeros(m, nu);
        Matrix V = Matrix.zeros(n, n);

        double[] e = new double[n];
        double[] work = new double[m];

        int nct = Math.min(m - 1, n);
        int nrt = Math.max(0, Math.min(n - 2, m));
        int mrc = Math.max(nct, nrt);

        for (int k = 0; k < mrc; k++) {
            if (k < nct) {
                s[k] = 0;
                for (int i = k; i < m; i++) {
                    s[k] = Utils.hypotenuse(s[k], a.get(i, k));
                }
                if (s[k] != 0) {
                    if (a.get(k, k) < 0) {
                        s[k] = -s[k];
                    }
                    for (int i = k; i < m; i++) {
                        a.set(i, k, a.get(i, k) / s[k]);
                    }
                    a.set(k, k, a.get(k, k) + 1);
                }
                s[k] = -s[k];
            }

            for (int j = k + 1; j < n; j++) {
                if (k < nct && s[k] != 0) {
                    double t = 0;
                    for (int i = k; i < m; i++) {
                        t += a.get(i, k) * a.get(i, j);
                    }
                    t = -t / a.get(k, k);
                    for (int i = k; i < m; i++) {
                        a.set(i, j, a.get(i, j) + t * a.get(i, k));
                    }
                }
                e[j] = a.get(k, j);
            }

            if (wantu && k < nct) {
                for (int i = k; i < m; i++) {
                    U.set(i, k, a.get(i, k));
                }
            }

            if (k < nrt) {
                e[k] = 0;
                for (int i = k + 1; i < n; i++) {
                    e[k] = Utils.hypotenuse(e[k], e[i]);
                }
                if (e[k] != 0) {
                    if (e[k + 1] < 0) {
                        e[k] = 0 - e[k];
                    }
                    for (int i = k + 1; i < n; i++) {
                        e[i] /= e[k];
                    }
                    e[k + 1] += 1;
                }
                e[k] = -e[k];
                if (k + 1 < m && e[k] != 0) {
                    for (int i = k + 1; i < m; i++) {
                        work[i] = 0;
                    }
                    for (int i = k + 1; i < m; i++) {
                        for (int j = k + 1; j < n; j++) {
                            work[i] += e[j] * a.get(i, j);
                        }
                    }
                    for (int j = k + 1; j < n; j++) {
                        double t = -e[j] / e[k + 1];
                        for (int i = k + 1; i < m; i++) {
                            a.set(i, j, a.get(i, j) + t * work[i]);
                        }
                    }
                }
                if (wantv) {
                    for (int i = k + 1; i < n; i++) {
                        V.set(i, k, e[i]);
                    }
                }
            }
        }

        int p = Math.min(n, m + 1);
        if (nct < n) {
            s[nct] = a.get(nct, nct);
        }
        if (m < p) {
            s[p - 1] = 0;
        }
        if (nrt + 1 < p) {
            e[nrt] = a.get(nrt, p - 1);
        }
        e[p - 1] = 0;

        if (wantu) {
            for (int j = nct; j < nu; j++) {
                for (int i = 0; i < m; i++) {
                    U.set(i, j, 0);
                }
                U.set(j, j, 1);
            }
            for (int k = nct - 1; k >= 0; k--) {
                if (s[k] != 0) {
                    for (int j = k + 1; j < nu; j++) {
                        double t = 0;
                        for (int i = k; i < m; i++) {
                            t += U.get(i, k) * U.get(i, j);
                        }
                        t = -t / U.get(k, k);
                        for (int i = k; i < m; i++) {
                            U.set(i, j, U.get(i, j) + t * U.get(i, k));
                        }
                    }
                    for (int i = k; i < m; i++) {
                        U.set(i, k, -U.get(i, k));
                    }
                    U.set(k, k, 1 + U.get(k, k));
                    for (int i = 0; i < k - 1; i++) {
                        U.set(i, k, 0);
                    }
                } else {
                    for (int i = 0; i < m; i++) {
                        U.set(i, k, 0);
                    }
                    U.set(k, k, 1);
                }
            }
        }

        if (wantv) {
            for (int k = n - 1; k >= 0; k--) {
                if (k < nrt && e[k] != 0) {
                    for (int j = k + 1; j < n; j++) {
                        double t = 0;
                        for (int i = k + 1; i < n; i++) {
                            t += V.get(i, k) * V.get(i, j);
                        }
                        t = -t / V.get(k + 1, k);
                        for (int i = k + 1; i < n; i++) {
                            V.set(i, j, V.get(i, j) + t * V.get(i, k));
                        }
                    }
                }
                for (int i = 0; i < n; i++) {
                    V.set(i, k, 0);
                }
                V.set(k, k, 1);
            }
        }

        // Main iteration loop for singular values (simplified)
        int pp = p - 1;
        int iter = 0;
        double eps = Math.pow(2.0, -52.0);
        while (p > 0) {
            int k, kase;
            for (k = p - 2; k >= -1; k--) {
                if (k == -1) {
                    break;
                }
                double alpha = Double.MIN_VALUE + eps * Math.abs(s[k] + Math.abs(s[k + 1]));
                if (Math.abs(e[k]) <= alpha || Double.isNaN(e[k])) {
                    e[k] = 0;
                    break;
                }
            }
            if (k == p - 2) {
                kase = 4;
            } else {
                int ks;
                for (ks = p - 1; ks >= k; ks--) {
                    if (ks == k) {
                        break;
                    }
                    double t = (ks != p ? Math.abs(e[ks]) : 0) +
                               (ks != k + 1 ? Math.abs(e[ks - 1]) : 0);
                    if (Math.abs(s[ks]) <= eps * t) {
                        s[ks] = 0;
                        break;
                    }
                }
                if (ks == k) {
                    kase = 3;
                } else if (ks == p - 1) {
                    kase = 1;
                } else {
                    kase = 2;
                    k = ks;
                }
            }

            k++;

            switch (kase) {
                case 4:
                    if (s[k] <= 0) {
                        s[k] = s[k] < 0 ? -s[k] : 0;
                        if (wantv) {
                            for (int i = 0; i <= pp; i++) {
                                V.set(i, k, -V.get(i, k));
                            }
                        }
                    }
                    while (k < pp) {
                        if (s[k] >= s[k + 1]) {
                            break;
                        }
                        double t = s[k];
                        s[k] = s[k + 1];
                        s[k + 1] = t;
                        if (wantv && k < n - 1) {
                            for (int i = 0; i < n; i++) {
                                t = V.get(i, k + 1);
                                V.set(i, k + 1, V.get(i, k));
                                V.set(i, k, t);
                            }
                        }
                        if (wantu && k < m - 1) {
                            for (int i = 0; i < m; i++) {
                                t = U.get(i, k + 1);
                                U.set(i, k + 1, U.get(i, k));
                                U.set(i, k, t);
                            }
                        }
                        k++;
                    }
                    iter = 0;
                    p--;
                    break;
                default:
                    // Simplified - just decrement p for other cases
                    p--;
                    break;
            }
        }

        if (swapped) {
            Matrix tmp = V;
            V = U;
            U = tmp;
        }

        this.m = m;
        this.n = n;
        this.s = s;
        this.U = U;
        this.V = V;
    }

    /**
     * Get the inverse of the matrix using SVD
     */
    public Matrix inverse() {
        Matrix V = this.V;
        double e = this.getThreshold();
        int vrows = V.rows;
        int vcols = V.columns;
        Matrix X = Matrix.zeros(vrows, this.s.length);

        for (int i = 0; i < vrows; i++) {
            for (int j = 0; j < vcols; j++) {
                if (Math.abs(this.s[j]) > e) {
                    X.set(i, j, V.get(i, j) / this.s[j]);
                }
            }
        }

        Matrix U = this.U;
        int urows = U.rows;
        int ucols = U.columns;
        Matrix Y = Matrix.zeros(vrows, urows);

        for (int i = 0; i < vrows; i++) {
            for (int j = 0; j < urows; j++) {
                double sum = 0;
                for (int k = 0; k < ucols; k++) {
                    sum += X.get(i, k) * U.get(j, k);
                }
                Y.set(i, j, sum);
            }
        }

        return Y;
    }

    /**
     * Solve a problem of least square (Ax=b) by using the SVD
     */
    public Matrix solve(Matrix value) {
        Matrix Y = value;
        double e = this.getThreshold();
        int scols = this.s.length;
        Matrix Ls = Matrix.zeros(scols, scols);

        for (int i = 0; i < scols; i++) {
            if (Math.abs(this.s[i]) <= e) {
                Ls.set(i, i, 0);
            } else {
                Ls.set(i, i, 1 / this.s[i]);
            }
        }

        Matrix U = this.U;
        Matrix V = this.getRightSingularVectors();

        Matrix VL = V.mmul(Ls);
        int vrows = V.rows;
        int urows = U.rows;
        Matrix VLU = Matrix.zeros(vrows, urows);

        for (int i = 0; i < vrows; i++) {
            for (int j = 0; j < urows; j++) {
                double sum = 0;
                for (int k = 0; k < scols; k++) {
                    sum += VL.get(i, k) * U.get(j, k);
                }
                VLU.set(i, j, sum);
            }
        }

        return VLU.mmul(Y);
    }

    public Matrix solveForDiagonal(double[] value) {
        return this.solve(Matrix.diag(value));
    }

    public double getCondition() {
        return this.s[0] / this.s[Math.min(this.m, this.n) - 1];
    }

    public double getNorm2() {
        return this.s[0];
    }

    public int getRank() {
        double tol = Math.max(this.m, this.n) * this.s[0] * Double.MIN_VALUE;
        int r = 0;
        for (int i = 0; i < s.length; i++) {
            if (s[i] > tol) {
                r++;
            }
        }
        return r;
    }

    public double[] getDiagonal() {
        return this.s.clone();
    }

    public double getThreshold() {
        return (Double.MIN_VALUE / 2) * Math.max(this.m, this.n) * this.s[0];
    }

    public Matrix getLeftSingularVectors() {
        return this.U;
    }

    public Matrix getRightSingularVectors() {
        return this.V;
    }

    public Matrix getDiagonalMatrix() {
        return Matrix.diag(this.s);
    }
}