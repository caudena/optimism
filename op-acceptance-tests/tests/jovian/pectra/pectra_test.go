package pectra

import (
	"testing"

	"github.com/ethereum-optimism/optimism/op-acceptance-tests/tests/jovian"
	"github.com/ethereum-optimism/optimism/op-devstack/devtest"
	"github.com/ethereum-optimism/optimism/op-devstack/presets"
	"github.com/ethereum-optimism/optimism/op-devstack/stack"
	"github.com/ethereum-optimism/optimism/op-devstack/sysgo"
)

func newSystem(gt *testing.T) *presets.Minimal {
	t := devtest.SerialT(gt)
	return presets.NewMinimal(t,
		stack.MakeCommon(sysgo.WithDeployerOptions(sysgo.WithJovianAtGenesis)),
	)
}

func TestDAFootprint(gt *testing.T) {
	jovian.TestDAFootprint(newSystem(gt))
}

func TestMinBaseFee(gt *testing.T) {
	jovian.TestMinBaseFee(newSystem(gt))
}

func TestOperatorFee(gt *testing.T) {
	jovian.TestOperatorFee(newSystem(gt))
}
