#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Domain {
    F32,
    F64,
}

#[derive(Clone, Copy, Debug)]
pub enum Constant {
    F32(f32),
    F64(f64),
}

impl Constant {
    pub fn domain(&self) -> Domain {
        match self {
            Constant::F32(_) => Domain::F32,
            Constant::F64(_) => Domain::F64,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn f32_constant_has_f32_domain() {
        let constant = Constant::F32(-0.0);

        let domain = constant.domain();

        assert_eq!(domain, Domain::F32);
    }

    #[test]
    fn f64_constant_has_f64_domain() {
        let constant = Constant::F64(f64::NAN);

        let domain = constant.domain();

        assert_eq!(domain, Domain::F64);
    }
}
