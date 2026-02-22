package custom_gas_token

import (
	"math/big"
	"testing"

	"github.com/ethereum/go-ethereum/common"

	"github.com/ethereum-optimism/optimism/op-devstack/devtest"
	"github.com/ethereum-optimism/optimism/op-devstack/presets"
	"github.com/ethereum-optimism/optimism/op-devstack/stack"
	"github.com/ethereum-optimism/optimism/op-devstack/sysgo"
)

func newCGTSystem(t devtest.T) *presets.Minimal {
	liq := new(big.Int).Mul(big.NewInt(1_000_000), big.NewInt(1e18))
	return presets.NewMinimal(t,
		stack.MakeCommon(sysgo.WithDeployerOptions(
			sysgo.WithCustomGasToken("Custom Gas Token", "CGT", liq, common.Address{}),
		)),
	)
}

// TestCGT_IntrospectionViaL1Block verifies that the L2 L1Block predeploy reports
// that CGT mode is enabled and exposes non-empty token metadata (name, symbol).
func TestCGT_IntrospectionViaL1Block(gt *testing.T) {
	t := devtest.SerialT(gt)
	sys := newCGTSystem(t)

	name, symbol := ensureCGTOrSkip(t, sys)

	// Metadata should be non-empty.
	if name == "" {
		t.Require().Fail("gasPayingTokenName() returned empty string")
	}
	if symbol == "" {
		t.Require().Fail("gasPayingTokenSymbol() returned empty string")
	}
}
