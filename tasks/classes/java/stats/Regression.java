package stats;

import java.util.Arrays;

/**
 * Determine the coefficient of determination (r^2) of a fit from the observations
 * and predictions.
 */
class StatUtils {
    public static double determinationCoefficient(double[][] data, double[] results) {
        double[] predictions = new double[data.length];
        double[] observations = new double[data.length];
        int validCount = 0;

        for (int i = 0; i < data.length; i++) {
            if (data[i][1] != Double.NaN) {
                observations[validCount] = data[i][1];
                predictions[validCount] = results[i];
                validCount++;
            }
        }

        double[] validObservations = Arrays.copyOf(observations, validCount);
        double[] validPredictions = Arrays.copyOf(predictions, validCount);

        double sum = 0;
        for (double obs : validObservations) {
            sum += obs;
        }
        double mean = sum / validObservations.length;

        double ssyy = 0;
        for (double obs : validObservations) {
            double difference = obs - mean;
            ssyy += difference * difference;
        }

        double sse = 0;
        for (int i = 0; i < validObservations.length; i++) {
            double residual = validObservations[i] - validPredictions[i];
            sse += residual * residual;
        }

        return 1 - (sse / ssyy);
    }

    /**
     * Determine the solution of a system of linear equations A * x = b using
     * Gaussian elimination.
     */
    public static double[] gaussianElimination(double[][] input, int order) {
        double[][] matrix = input;
        int n = input.length - 1;
        double[] coefficients = new double[order];

        for (int i = 0; i < n; i++) {
            int maxrow = i;
            for (int j = i + 1; j < n; j++) {
                if (Math.abs(matrix[i][j]) > Math.abs(matrix[i][maxrow])) {
                    maxrow = j;
                }
            }

            for (int k = i; k < n + 1; k++) {
                double tmp = matrix[k][i];
                matrix[k][i] = matrix[k][maxrow];
                matrix[k][maxrow] = tmp;
            }

            for (int j = i + 1; j < n; j++) {
                for (int k = n; k >= i; k--) {
                    matrix[k][j] -= (matrix[k][i] * matrix[i][j]) / matrix[i][i];
                }
            }
        }

        for (int j = n - 1; j >= 0; j--) {
            double total = 0;
            for (int k = j + 1; k < n; k++) {
                total += matrix[k][j] * coefficients[k];
            }
            coefficients[j] = (matrix[n][j] - total) / matrix[j][j];
        }

        return coefficients;
    }

    /**
     * Round a number to a precision, specified in number of decimal places
     */
    public static double round(double number, int precision) {
        double factor = Math.pow(10, precision);
        return Math.round(number * factor) / factor;
    }
}

public class RegressionOptions {
    public int precision;
    public int order;

    public RegressionOptions(int precision, int order) {
        this.precision = precision;
        this.order = order;
    }
}

public class LinearRegression {
    public double gradient;
    public double intercept;
    public double[] points;
    public String string;
    public double r2;
    public RegressionOptions options;
    public double[] equation;

    public LinearRegression(double[][] data) {
        this.options = new RegressionOptions(2, 2);

        double[] sum = new double[5];
        int len = 0;

        for (int n = 0; n < data.length; n++) {
            if (!Double.isNaN(data[n][1])) {
                len++;
                sum[0] += data[n][0];
                sum[1] += data[n][1];
                sum[2] += data[n][0] * data[n][0];
                sum[3] += data[n][0] * data[n][1];
                sum[4] += data[n][1] * data[n][1];
            }
        }

        double run = (len * sum[2]) - (sum[0] * sum[0]);
        double rise = (len * sum[3]) - (sum[0] * sum[1]);
        double gradient = run == 0 ? 0 : StatUtils.round(rise / run, this.options.precision);
        double intercept = StatUtils.round((sum[1] / len) - ((gradient * sum[0]) / len), this.options.precision);

        this.gradient = gradient;
        this.intercept = intercept;
        this.points = new double[data.length];
        for (int i = 0; i < data.length; i++) {
            this.points[i] = this.predict(data[i][0]);
        }
        this.equation = new double[]{gradient, intercept};
        this.r2 = StatUtils.round(StatUtils.determinationCoefficient(data, this.points), this.options.precision);
        this.string = intercept == 0 ? "y = " + gradient + "x" : "y = " + gradient + "x + " + intercept;
    }

    public double predict(double x) {
        return StatUtils.round((this.gradient * x) + this.intercept, this.options.precision);
    }
}

public class ExponentialRegression {
    public double coeffA;
    public double coeffB;
    public double[] points;
    public String string;
    public double r2;
    public RegressionOptions options;
    public double[] equation;

    public ExponentialRegression(double[][] data) {
        this.options = new RegressionOptions(2, 2);
        double[] sum = new double[6];

        for (int n = 0; n < data.length; n++) {
            if (!Double.isNaN(data[n][1])) {
                sum[0] += data[n][0];
                sum[1] += data[n][1];
                sum[2] += data[n][0] * data[n][0] * data[n][1];
                sum[3] += data[n][1] * Math.log(data[n][1]);
                sum[4] += data[n][0] * data[n][1] * Math.log(data[n][1]);
                sum[5] += data[n][0] * data[n][1];
            }
        }

        double denominator = (sum[1] * sum[2]) - (sum[5] * sum[5]);
        double a = Math.exp(((sum[2] * sum[3]) - (sum[5] * sum[4])) / denominator);
        double b = ((sum[1] * sum[4]) - (sum[5] * sum[3])) / denominator;

        this.coeffA = StatUtils.round(a, this.options.precision);
        this.coeffB = StatUtils.round(b, this.options.precision);
        this.points = new double[data.length];
        for (int i = 0; i < data.length; i++) {
            this.points[i] = this.predict(data[i][0]);
        }
        this.equation = new double[]{this.coeffA, this.coeffB};
        this.string = "y = " + this.coeffA + "e^(" + this.coeffB + "x)";
        this.r2 = StatUtils.round(StatUtils.determinationCoefficient(data, this.points), this.options.precision);
    }

    public double predict(double x) {
        return StatUtils.round(this.coeffA * Math.exp(this.coeffB * x), this.options.precision);
    }
}

public class LogarithmicRegression {
    public double coeffA;
    public double coeffB;
    public double[] points;
    public String string;
    public double r2;
    public RegressionOptions options;
    public double[] equation;

    public LogarithmicRegression(double[][] data) {
        this.options = new RegressionOptions(2, 2);
        double[] sum = new double[4];
        int len = data.length;
        
        for (int n = 0; n < len; n++) {
            if (!Double.isNaN(data[n][1])) {
                sum[0] += Math.log(data[n][0]);
                sum[1] += data[n][1] * Math.log(data[n][0]);
                sum[2] += data[n][1];
                sum[3] += Math.log(data[n][0]) * Math.log(data[n][0]);
            }
        }

        double a = ((len * sum[1]) - (sum[2] * sum[0])) / ((len * sum[3]) - (sum[0] * sum[0]));
        this.coeffB = StatUtils.round(a, this.options.precision);
        this.coeffA = StatUtils.round((sum[2] - (this.coeffB * sum[0])) / len, this.options.precision);

        this.points = new double[data.length];
        for (int i = 0; i < data.length; i++) {
            this.points[i] = this.predict(data[i][0]);
        }
        this.equation = new double[]{this.coeffA, this.coeffB};
        this.string = "y = " + this.coeffA + " + " + this.coeffB + " ln(x)";
        this.r2 = StatUtils.round(StatUtils.determinationCoefficient(data, this.points), this.options.precision);
    }

    public double predict(double x) {
        return StatUtils.round(this.coeffA + (this.coeffB * Math.log(x)), this.options.precision);
    }
}

public class PowerRegression {
    public double coeffA;
    public double coeffB;
    public double[] points;
    public String string;
    public double r2;
    public RegressionOptions options;
    public double[] equation;

    public PowerRegression(double[][] data) {
        this.options = new RegressionOptions(2, 2);
        double[] sum = new double[5];
        int len = data.length;

        for (int n = 0; n < len; n++) {
            if (!Double.isNaN(data[n][1])) {
                sum[0] += Math.log(data[n][0]);
                sum[1] += Math.log(data[n][1]) * Math.log(data[n][0]);
                sum[2] += Math.log(data[n][1]);
                sum[3] += Math.log(data[n][0]) * Math.log(data[n][0]);
            }
        }

        double b = ((len * sum[1]) - (sum[0] * sum[2])) / ((len * sum[3]) - (sum[0] * sum[0]));
        double a = (sum[2] - (b * sum[0])) / len;
        this.coeffA = StatUtils.round(Math.exp(a), this.options.precision);
        this.coeffB = StatUtils.round(b, this.options.precision);

        this.points = new double[data.length];
        for (int i = 0; i < data.length; i++) {
            this.points[i] = this.predict(data[i][0]);
        }
        this.equation = new double[]{this.coeffA, this.coeffB};
        this.string = "y = " + this.coeffA + "x^" + this.coeffB;
        this.r2 = StatUtils.round(StatUtils.determinationCoefficient(data, this.points), this.options.precision);
    }

    public double predict(double x) {
        return StatUtils.round(this.coeffA * Math.pow(x, this.coeffB), this.options.precision);
    }
}

public class PolynomialRegression {
    public double[] coefficients;
    public double[] points;
    public String string;
    public double r2;
    public RegressionOptions options;
    public double[] equation;

    public PolynomialRegression(double[][] data) {
        this.options = new RegressionOptions(2, 2);
        double[][] lhs = new double[this.options.order + 1][];
        double[] rhs = new double[this.options.order + 1];
        double a = 0;
        double b = 0;
        int len = data.length;
        int k = this.options.order + 1;

        for (int i = 0; i < k; i++) {
            for (int l = 0; l < len; l++) {
                if (!Double.isNaN(data[l][1])) {
                    a += Math.pow(data[l][0], i) * data[l][1];
                }
            }

            rhs[i] = a;
            a = 0;

            double[] c = new double[k];
            for (int j = 0; j < k; j++) {
                for (int l = 0; l < len; l++) {
                    if (!Double.isNaN(data[l][1])) {
                        b += Math.pow(data[l][0], i + j);
                    }
                }
                c[j] = b;
                b = 0;
            }
            lhs[i] = c;
        }

        // Create augmented matrix for Gaussian elimination
        double[][] augmented = new double[k + 1][k];
        for (int i = 0; i < k; i++) {
            System.arraycopy(lhs[i], 0, augmented[i], 0, k);
        }
        System.arraycopy(rhs, 0, augmented[k], 0, k);

        double[] coefficients = StatUtils.gaussianElimination(augmented, k);
        for (int i = 0; i < coefficients.length; i++) {
            coefficients[i] = StatUtils.round(coefficients[i], this.options.precision);
        }

        this.coefficients = coefficients;
        this.points = new double[data.length];
        for (int i = 0; i < data.length; i++) {
            this.points[i] = this.predict(data[i][0]);
        }

        StringBuilder sb = new StringBuilder("y = ");
        for (int i = coefficients.length - 1; i >= 0; i--) {
            if (i > 1) {
                sb.append(coefficients[i]).append("x^").append(i).append(" + ");
            } else if (i == 1) {
                sb.append(coefficients[i]).append("x + ");
            } else {
                sb.append(coefficients[i]);
            }
        }

        this.string = sb.toString();
        this.equation = coefficients.clone();
        // Reverse for equation array
        for (int i = 0; i < this.equation.length / 2; i++) {
            double temp = this.equation[i];
            this.equation[i] = this.equation[this.equation.length - 1 - i];
            this.equation[this.equation.length - 1 - i] = temp;
        }
        this.r2 = StatUtils.round(StatUtils.determinationCoefficient(data, this.points), this.options.precision);
    }

    public double predict(double x) {
        double sum = 0;
        for (int i = 0; i < this.coefficients.length; i++) {
            sum += this.coefficients[i] * Math.pow(x, i);
        }
        return StatUtils.round(sum, this.options.precision);
    }
}