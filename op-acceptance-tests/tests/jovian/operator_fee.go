package jovian

import (
	"github.com/ethereum-optimism/optimism/op-core/forks"
	"github.com/ethereum-optimism/optimism/op-devstack/dsl"
	"github.com/ethereum-optimism/optimism/op-devstack/presets"
)

func TestOperatorFee(sys *presets.Minimal) {
	t := sys.T
	t.Require().True(sys.L2Chain.IsForkActive(forks.Jovian), "Jovian fork must be active for this test")
	dsl.RunOperatorFeeTest(t, sys.L2Chain, sys.L1EL, sys.FunderL1, sys.FunderL2)
}
