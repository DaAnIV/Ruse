class Student {
    constructor(public name: string,
        public surname: string,
        public age: number,
        public grades: number[]) { }
}

class Class {
    constructor(public students: Student[]) { }
}

export {
    Student,
    Class
};
